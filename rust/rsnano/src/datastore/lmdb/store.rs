use std::{ffi::CString, path::Path, sync::Arc, time::Duration};

use crate::{
    datastore::{
        lmdb::{mdb_env_copy, MDB_SUCCESS},
        Store, Transaction, VersionStore, WriteTransaction, STORE_VERSION_MINIMUM,
    },
    logger_mt::Logger,
    utils::seconds_since_epoch,
    LmdbConfig, TxnTrackingConfig,
};

use super::{
    ensure_success, mdb_env_copy2, EnvOptions, LmdbAccountStore, LmdbBlockStore,
    LmdbConfirmationHeightStore, LmdbEnv, LmdbFinalVoteStore, LmdbFrontierStore,
    LmdbOnlineWeightStore, LmdbPeerStore, LmdbPendingStore, LmdbPrunedStore, LmdbUncheckedStore,
    LmdbVersionStore, MDB_CP_COMPACT,
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
    ) -> anyhow::Result<Self> {
        let env = Arc::new(LmdbEnv::with_tracking(
            path,
            options,
            tracking_cfg,
            block_processor_batch_max_time,
            logger.clone(),
        )?);

        Ok(Self {
            env: env.clone(),
            block_store: Arc::new(LmdbBlockStore::new(env.clone())),
            frontier_store: Arc::new(LmdbFrontierStore::new(env.clone())),
            account_store: Arc::new(LmdbAccountStore::new(env.clone())),
            pending_store: Arc::new(LmdbPendingStore::new(env.clone())),
            online_weight_store: Arc::new(LmdbOnlineWeightStore::new(env.clone())),
            pruned_store: Arc::new(LmdbPrunedStore::new(env.clone())),
            peer_store: Arc::new(LmdbPeerStore::new(env.clone())),
            confirmation_height_store: Arc::new(LmdbConfirmationHeightStore::new(env.clone())),
            final_vote_store: Arc::new(LmdbFinalVoteStore::new(env.clone())),
            unchecked_store: Arc::new(LmdbUncheckedStore::new(env.clone())),
            version_store: Arc::new(LmdbVersionStore::new(env.clone())),
            logger,
        })
    }

    pub fn open_databases(&self, txn: &dyn Transaction, flags: u32) -> anyhow::Result<()> {
        self.block_store.open_db(txn, flags)?;
        self.frontier_store.open_db(txn, flags)?;
        self.account_store.open_db(txn, flags)?;
        self.pending_store.open_db(txn, flags)?;
        self.online_weight_store.open_db(txn, flags)?;
        self.pruned_store.open_db(txn, flags)?;
        self.peer_store.open_db(txn, flags)?;
        self.confirmation_height_store.open_db(txn, flags)?;
        self.final_vote_store.open_db(txn, flags)?;
        self.unchecked_store.open_db(txn, flags)?;
        self.version_store.open_db(txn, flags)
    }

    pub fn do_upgrades(&self, txn: &dyn WriteTransaction) -> anyhow::Result<Vacuuming> {
        let version = self.version_store.get(txn.as_transaction());
        match version {
            1..=20 => {
                self.logger.always_log(&format!("The version of the ledger ({}) is lower than the minimum ({}) which is supported for upgrades. Either upgrade to a v23 node first or delete the ledger.", version, STORE_VERSION_MINIMUM));
                Err(anyhow!("version too low"))
            }
            21 => {
                // most recent version
                Ok(Vacuuming::NotNeeded)
            }
            _ => {
                self.logger.always_log(&format!(
                    "The version of the ledger ({}) is too high for this node",
                    version
                ));
                Err(anyhow!("version too high"))
            }
        }
    }

    pub fn vacuum_after_upgrade(&self, path: &Path, config: LmdbConfig) -> anyhow::Result<()> {
        // Vacuum the database. This is not a required step and may actually fail if there isn't enough storage space.
        let mut vacuum_path = path.to_owned();
        vacuum_path.pop();
        vacuum_path.push("vacuumed.ldb");

        match self.copy_db(&vacuum_path) {
            Ok(_) => {
                self.env.close();

                // Replace the ledger file with the vacuumed one
                std::fs::rename(&vacuum_path, path)?;

                // Set up the environment again
                let options = EnvOptions {
                    config,
                    use_no_mem_init: true,
                };
                self.env.init(path, &options)?;
                let transaction = self.env.tx_begin_read();
                self.open_databases(&transaction, 0)
            }
            Err(e) => {
                // The vacuum file can be in an inconsistent state if there wasn't enough space to create it
                let _ = std::fs::remove_file(&vacuum_path);
                Err(e)
            }
        }
    }
}

impl Store for LmdbStore {
    fn copy_db(&self, destination: &Path) -> anyhow::Result<()> {
        let c_path = CString::new(destination.as_os_str().to_str().unwrap()).unwrap();
        let status = unsafe { mdb_env_copy2(self.env.env(), c_path.as_ptr(), MDB_CP_COMPACT) };
        ensure_success(status)
    }
}

/// Takes a filepath, appends '_backup_<timestamp>' to the end (but before any extension) and saves that file in the same directory
pub fn create_backup_file(
    env: &LmdbEnv,
    source_path: &Path,
    logger: &dyn Logger,
) -> anyhow::Result<()> {
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
    let status = unsafe { mdb_env_copy(env.env(), backup_path_cstr.as_ptr()) };
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
