use std::{
    ffi::{c_char, CStr, CString},
    path::{Path, PathBuf},
    ptr,
    sync::Arc,
    time::Duration,
};

use lmdb::{Cursor, Database, DatabaseFlags, Transaction, WriteFlags};
use lmdb_sys::{MDB_CP_COMPACT, MDB_SUCCESS};

use crate::{
    datastore::{Store, VersionStore, STORE_VERSION_CURRENT, STORE_VERSION_MINIMUM},
    logger_mt::Logger,
    utils::{seconds_since_epoch, PropertyTreeWriter},
    TxnTrackingConfig,
};

use super::{
    EnvOptions, LmdbAccountStore, LmdbBlockStore, LmdbConfirmationHeightStore, LmdbEnv,
    LmdbFinalVoteStore, LmdbFrontierStore, LmdbOnlineWeightStore, LmdbPeerStore, LmdbPendingStore,
    LmdbPrunedStore, LmdbReadTransaction, LmdbUncheckedStore, LmdbVersionStore,
    LmdbWriteTransaction,
};

#[derive(PartialEq, Eq)]
pub enum Vacuuming {
    Needed,
    NotNeeded,
}

pub struct LmdbStore {
    pub env: Arc<LmdbEnv>,
    pub block_store: Arc<LmdbBlockStore>,
    pub frontier_store: Arc<LmdbFrontierStore>,
    pub account_store: Arc<LmdbAccountStore>,
    pub pending_store: Arc<LmdbPendingStore>,
    pub online_weight_store: Arc<LmdbOnlineWeightStore>,
    pub pruned_store: Arc<LmdbPrunedStore>,
    pub peer_store: Arc<LmdbPeerStore>,
    pub confirmation_height_store: Arc<LmdbConfirmationHeightStore>,
    pub final_vote_store: Arc<LmdbFinalVoteStore>,
    pub unchecked_store: Arc<LmdbUncheckedStore>,
    pub version_store: Arc<LmdbVersionStore>,
    logger: Arc<dyn Logger>,
}

impl LmdbStore {
    pub fn new(
        path: &Path,
        options: &EnvOptions,
        tracking_cfg: TxnTrackingConfig,
        block_processor_batch_max_time: Duration,
        logger: Arc<dyn Logger>,
        backup_before_upgrade: bool,
    ) -> anyhow::Result<Self> {
        let mut is_fully_upgraded = false;
        let mut is_fresh_db = false;
        {
            let env = LmdbEnv::new(path)?;
            match LmdbVersionStore::try_read_version(&env) {
                Some(version) => is_fully_upgraded = version == STORE_VERSION_CURRENT,
                None => is_fresh_db = true,
            }
        }

        // Only open a write lock when upgrades are needed. This is because CLI commands
        // open inactive nodes which can otherwise be locked here if there is a long write
        // (can be a few minutes with the --fast_bootstrap flag for instance)
        if !is_fully_upgraded {
            let env = Arc::new(LmdbEnv::new(path)?);
            if !is_fresh_db {
                logger.always_log("Upgrade in progress...");
                if backup_before_upgrade {
                    create_backup_file(&env, logger.as_ref())?;
                }
            }

            let vacuuming = do_upgrades(env.clone(), logger.as_ref())?;
            if vacuuming == Vacuuming::Needed {
                logger.always_log("Preparing vacuum...");
                match vacuum_after_upgrade (env, path){
                    Ok(_) => logger.always_log("Vacuum succeeded."),
                    Err(_) => logger.always_log("Failed to vacuum. (Optional) Ensure enough disk space is available for a copy of the database and try to vacuum after shutting down the node"),
                }
            }
        }

        let env = Arc::new(LmdbEnv::with_tracking(
            path,
            options,
            tracking_cfg,
            block_processor_batch_max_time,
            logger.clone(),
        )?);

        Ok(Self {
            block_store: Arc::new(LmdbBlockStore::new(env.clone())?),
            frontier_store: Arc::new(LmdbFrontierStore::new(env.clone())?),
            account_store: Arc::new(LmdbAccountStore::new(env.clone())?),
            pending_store: Arc::new(LmdbPendingStore::new(env.clone())?),
            online_weight_store: Arc::new(LmdbOnlineWeightStore::new(env.clone())?),
            pruned_store: Arc::new(LmdbPrunedStore::new(env.clone())?),
            peer_store: Arc::new(LmdbPeerStore::new(env.clone())?),
            confirmation_height_store: Arc::new(LmdbConfirmationHeightStore::new(env.clone())?),
            final_vote_store: Arc::new(LmdbFinalVoteStore::new(env.clone())?),
            unchecked_store: Arc::new(LmdbUncheckedStore::new(env.clone())?),
            version_store: Arc::new(LmdbVersionStore::new(env.clone())?),
            logger,
            env,
        })
    }

    pub fn rebuild_db(&self, txn: &mut LmdbWriteTransaction) -> anyhow::Result<()> {
        let tables = [
            self.account_store.database(),
            self.block_store.database(),
            self.pruned_store.database(),
            self.confirmation_height_store.database(),
            self.pending_store.database(),
        ];
        for table in tables {
            rebuild_table(&self.env, txn, table)?;
        }

        Ok(())
    }

    pub fn serialize_memory_stats(&self, json: &mut dyn PropertyTreeWriter) -> anyhow::Result<()> {
        let stats = self.env.environment.stat()?;
        json.put_u64("branch_pages", stats.branch_pages() as u64)?;
        json.put_u64("depth", stats.depth() as u64)?;
        json.put_u64("entries", stats.entries() as u64)?;
        json.put_u64("leaf_pages", stats.leaf_pages() as u64)?;
        json.put_u64("overflow_pages", stats.overflow_pages() as u64)?;
        json.put_u64("page_size", stats.page_size() as u64)?;
        Ok(())
    }

    pub fn vendor(&self) -> String {
        // fake version! todo: read version
        format!("lmdb-rkv {}.{}.{}", 0, 14, 0)
    }

    pub fn serialize_mdb_tracker(
        &self,
        json: &mut dyn PropertyTreeWriter,
        min_read_time: Duration,
        min_write_time: Duration,
    ) -> anyhow::Result<()> {
        self.env
            .serialize_txn_tracker(json, min_read_time, min_write_time)
    }

    pub fn tx_begin_read(&self) -> lmdb::Result<LmdbReadTransaction> {
        self.env.tx_begin_read()
    }

    pub fn tx_begin_write(&self) -> lmdb::Result<LmdbWriteTransaction> {
        self.env.tx_begin_write()
    }
}

fn rebuild_table(
    env: &LmdbEnv,
    rw_txn: &mut LmdbWriteTransaction,
    db: Database,
) -> anyhow::Result<()> {
    let temp = {
        let ro_txn = env.tx_begin_read()?;
        let temp = unsafe {
            rw_txn
                .rw_txn_mut()
                .create_db(Some("temp_table"), DatabaseFlags::empty())
        }?;
        // Copy all values to temporary table
        {
            {
                let mut cursor = ro_txn.txn().open_ro_cursor(db)?;
                for x in cursor.iter_start() {
                    let (k, v) = x?;
                    rw_txn.rw_txn_mut().put(temp, &k, &v, WriteFlags::APPEND)?;
                }
            }

            if ro_txn.txn().stat(db)?.entries() != rw_txn.rw_txn_mut().stat(temp)?.entries() {
                bail!("table count mismatch");
            }
        }
        rw_txn.refresh();
        temp
    };

    // Put values from copy
    {
        let ro_txn = env.tx_begin_read()?;
        rw_txn.rw_txn_mut().clear_db(db)?;
        {
            let mut cursor = ro_txn.txn().open_ro_cursor(temp)?;
            for x in cursor.iter_start() {
                let (k, v) = x?;
                rw_txn.rw_txn_mut().put(db, &k, &v, WriteFlags::APPEND)?;
            }
        }
        if rw_txn.rw_txn_mut().stat(db)?.entries() != ro_txn.txn().stat(temp)?.entries() {
            bail!("table count mismatch");
        }
    }

    unsafe { rw_txn.rw_txn_mut().drop_db(temp) }?;
    rw_txn.refresh();
    Ok(())
}

fn do_upgrades(env: Arc<LmdbEnv>, logger: &dyn Logger) -> anyhow::Result<Vacuuming> {
    let version_store = LmdbVersionStore::new(env.clone())?;
    let txn = env.tx_begin_write()?;
    let version = version_store.get(&txn.as_txn());
    match version {
        1..=20 => {
            logger.always_log(&format!("The version of the ledger ({}) is lower than the minimum ({}) which is supported for upgrades. Either upgrade to a v23 node first or delete the ledger.", version, STORE_VERSION_MINIMUM));
            Err(anyhow!("version too low"))
        }
        21 => {
            // most recent version
            Ok(Vacuuming::NotNeeded)
        }
        _ => {
            logger.always_log(&format!(
                "The version of the ledger ({}) is too high for this node",
                version
            ));
            Err(anyhow!("version too high"))
        }
    }
}

fn vacuum_after_upgrade(env: Arc<LmdbEnv>, path: &Path) -> anyhow::Result<()> {
    // Vacuum the database. This is not a required step and may actually fail if there isn't enough storage space.
    let mut vacuum_path = path.to_owned();
    vacuum_path.pop();
    vacuum_path.push("vacuumed.ldb");

    match copy_db(&env, &vacuum_path) {
        Ok(_) => {
            //todo don't use Arc here! Env must be dropped!
            drop(env);

            // Replace the ledger file with the vacuumed one
            std::fs::rename(&vacuum_path, path)?;
            Ok(())
        }
        Err(e) => {
            // The vacuum file can be in an inconsistent state if there wasn't enough space to create it
            let _ = std::fs::remove_file(&vacuum_path);
            Err(e)
        }
    }
}
fn copy_db(env: &LmdbEnv, destination: &Path) -> anyhow::Result<()> {
    let c_path = CString::new(destination.as_os_str().to_str().unwrap()).unwrap();
    let status =
        unsafe { lmdb_sys::mdb_env_copy2(env.environment.env(), c_path.as_ptr(), MDB_CP_COMPACT) };
    ensure_success(status)
}

impl Store for LmdbStore {
    fn copy_db(&self, destination: &Path) -> anyhow::Result<()> {
        copy_db(&self.env, destination)
    }
}

fn ensure_success(status: i32) -> Result<(), anyhow::Error> {
    if status == MDB_SUCCESS {
        Ok(())
    } else {
        Err(anyhow!("lmdb returned status code {}", status))
    }
}

/// Takes a filepath, appends '_backup_<timestamp>' to the end (but before any extension) and saves that file in the same directory
pub fn create_backup_file(env: &LmdbEnv, logger: &dyn Logger) -> anyhow::Result<()> {
    let mut path: *const c_char = ptr::null();
    let status = unsafe { lmdb_sys::mdb_env_get_path(env.environment.env(), &mut path) };
    if status != MDB_SUCCESS {
        bail!("could not get env path");
    }
    let source_path: PathBuf = unsafe { CStr::from_ptr(path) }.to_str()?.into();

    let extension = source_path
        .extension()
        .ok_or_else(|| anyhow!("no extension"))?
        .to_string_lossy();
    let file_name = source_path
        .file_name()
        .ok_or_else(|| anyhow!("no file name"))?
        .to_string_lossy();
    let file_stem = source_path
        .file_stem()
        .ok_or_else(|| anyhow!("no file stem"))?
        .to_string_lossy();
    let mut backup_path = source_path
        .parent()
        .ok_or_else(|| anyhow!("no parent path"))?
        .to_owned();
    let backup_filename = format!(
        "{}_backup_{}.{}",
        file_stem,
        seconds_since_epoch(),
        extension
    );
    backup_path.push(&backup_filename);

    let start_message = format!("Performing {} backup before database upgrade...", file_name);
    logger.always_log(&start_message);
    println!("{}", start_message);

    let backup_path_cstr = CString::new(
        backup_path
            .as_os_str()
            .to_str()
            .ok_or_else(|| anyhow!("invalid backup path"))?,
    )?;
    let status =
        unsafe { lmdb_sys::mdb_env_copy(env.environment.env(), backup_path_cstr.as_ptr()) };
    if status != MDB_SUCCESS {
        let error_message = format!("{} backup failed", file_name);
        logger.always_log(&error_message);
        eprintln!("{}", error_message);
        Err(anyhow!(error_message))
    } else {
        let success_message = format!("Backup created: {}", backup_filename);
        logger.always_log(&success_message);
        println!("{}", success_message);
        Ok(())
    }
}
