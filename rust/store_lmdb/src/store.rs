use std::{
    ffi::CString,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{
    lmdb_env::{EnvironmentWrapper, RoTransactionStrategy},
    EnvOptions, EnvironmentStrategy, LmdbAccountStore, LmdbBlockStore, LmdbConfirmationHeightStore,
    LmdbEnv, LmdbFinalVoteStore, LmdbFrontierStore, LmdbOnlineWeightStore, LmdbPeerStore,
    LmdbPendingStore, LmdbPrunedStore, LmdbReadTransaction, LmdbVersionStore, LmdbWriteTransaction,
    NullTransactionTracker, Table, TransactionTracker, STORE_VERSION_MINIMUM,
};
use lmdb::{Cursor, Database, DatabaseFlags, Transaction, WriteFlags};
use lmdb_sys::{MDB_CP_COMPACT, MDB_SUCCESS};
use rsnano_core::utils::{seconds_since_epoch, Logger, NullLogger, PropertyTreeWriter};

#[derive(PartialEq, Eq)]
pub enum Vacuuming {
    Needed,
    NotNeeded,
}

pub struct LmdbStore<T: EnvironmentStrategy = EnvironmentWrapper> {
    pub env: Arc<LmdbEnv<T>>,
    pub block: Arc<LmdbBlockStore<T>>,
    pub frontier: Arc<LmdbFrontierStore<T>>,
    pub account: Arc<LmdbAccountStore<T>>,
    pub pending: Arc<LmdbPendingStore<T>>,
    pub online_weight: Arc<LmdbOnlineWeightStore<T>>,
    pub pruned: Arc<LmdbPrunedStore<T>>,
    pub peer: Arc<LmdbPeerStore<T>>,
    pub confirmation_height: Arc<LmdbConfirmationHeightStore<T>>,
    pub final_vote: Arc<LmdbFinalVoteStore<T>>,
    pub version: Arc<LmdbVersionStore<T>>,
}

pub struct LmdbStoreBuilder<'a> {
    path: &'a Path,
    options: Option<&'a EnvOptions>,
    tracker: Option<Arc<dyn TransactionTracker>>,
    logger: Option<Arc<dyn Logger>>,
    backup_before_upgrade: bool,
}

impl<'a> LmdbStoreBuilder<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            options: None,
            tracker: None,
            logger: None,
            backup_before_upgrade: false,
        }
    }

    pub fn options(mut self, options: &'a EnvOptions) -> Self {
        self.options = Some(options);
        self
    }

    pub fn txn_tracker(mut self, tracker: Arc<dyn TransactionTracker>) -> Self {
        self.tracker = Some(tracker);
        self
    }

    pub fn logger(mut self, logger: Arc<dyn Logger>) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn backup_before_upgrade(mut self, backup: bool) -> Self {
        self.backup_before_upgrade = backup;
        self
    }

    pub fn build(self) -> anyhow::Result<LmdbStore<EnvironmentWrapper>> {
        let default_options = Default::default();
        let options = self.options.unwrap_or(&default_options);

        let txn_tracker = self
            .tracker
            .unwrap_or_else(|| Arc::new(NullTransactionTracker::new()));

        let logger = self.logger.unwrap_or_else(|| Arc::new(NullLogger::new()));

        LmdbStore::new(
            self.path,
            options,
            txn_tracker,
            logger,
            self.backup_before_upgrade,
        )
    }
}

impl<T: EnvironmentStrategy + 'static> LmdbStore<T> {
    pub fn open<'a>(path: &'a Path) -> LmdbStoreBuilder<'a> {
        LmdbStoreBuilder::new(path)
    }

    fn new(
        path: impl AsRef<Path>,
        options: &EnvOptions,
        txn_tracker: Arc<dyn TransactionTracker>,
        logger: Arc<dyn Logger>,
        backup_before_upgrade: bool,
    ) -> anyhow::Result<Self> {
        let path = path.as_ref();
        upgrade_if_needed::<T>(path, &logger, backup_before_upgrade)?;

        let env = Arc::new(LmdbEnv::<T>::with_txn_tracker(path, options, txn_tracker)?);

        Ok(Self {
            block: Arc::new(LmdbBlockStore::new(env.clone())?),
            frontier: Arc::new(LmdbFrontierStore::new(env.clone())?),
            account: Arc::new(LmdbAccountStore::new(env.clone())?),
            pending: Arc::new(LmdbPendingStore::new(env.clone())?),
            online_weight: Arc::new(LmdbOnlineWeightStore::new(env.clone())?),
            pruned: Arc::new(LmdbPrunedStore::new(env.clone())?),
            peer: Arc::new(LmdbPeerStore::new(env.clone())?),
            confirmation_height: Arc::new(LmdbConfirmationHeightStore::new(env.clone())?),
            final_vote: Arc::new(LmdbFinalVoteStore::new(env.clone())?),
            version: Arc::new(LmdbVersionStore::new(env.clone())?),
            env,
        })
    }
}

impl<T: EnvironmentStrategy + 'static> LmdbStore<T> {
    pub fn copy_db(&self, destination: &Path) -> anyhow::Result<()> {
        copy_db(&self.env, destination)
    }

    pub fn tx_begin_write_for(&self, _to_lock: &[Table]) -> LmdbWriteTransaction<T> {
        // locking tables is not needed for LMDB because there can only ever be one write transaction at a time
        self.env
            .tx_begin_write()
            .expect("Could not create LMDB read/write transaction")
    }

    pub fn rebuild_db(&self, txn: &mut LmdbWriteTransaction) -> anyhow::Result<()> {
        let tables = [
            self.account.database(),
            self.block.database(),
            self.pruned.database(),
            self.confirmation_height.database(),
            self.pending.database(),
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

    pub fn tx_begin_read(&self) -> LmdbReadTransaction<T> {
        self.env.tx_begin_read().unwrap()
    }

    pub fn tx_begin_write(&self) -> LmdbWriteTransaction<T> {
        self.env.tx_begin_write().unwrap()
    }
}

fn upgrade_if_needed<T: EnvironmentStrategy + 'static>(
    path: &Path,
    logger: &Arc<dyn Logger>,
    backup_before_upgrade: bool,
) -> Result<(), anyhow::Error> {
    let upgrade_info = LmdbVersionStore::<T>::check_upgrade(path)?;
    if upgrade_info.is_fully_upgraded {
        logger.always_log("No database upgrade needed");
        return Ok(());
    }

    let env = Arc::new(LmdbEnv::new(path)?);
    if !upgrade_info.is_fresh_db {
        if backup_before_upgrade {
            create_backup_file(&env, logger.as_ref())?;
        }
    }

    logger.always_log("Upgrade in progress...");
    let vacuuming = do_upgrades(env.clone(), logger.as_ref())?;
    logger.always_log("Upgrade done!");

    if vacuuming == Vacuuming::Needed {
        logger.always_log("Preparing vacuum...");
        match vacuum_after_upgrade (env, path){
                Ok(_) => logger.always_log("Vacuum succeeded."),
                Err(_) => logger.always_log("Failed to vacuum. (Optional) Ensure enough disk space is available for a copy of the database and try to vacuum after shutting down the node"),
            }
    }
    Ok(())
}

fn rebuild_table<T: EnvironmentStrategy + 'static>(
    env: &LmdbEnv<T>,
    rw_txn: &mut LmdbWriteTransaction,
    db: Database,
) -> anyhow::Result<()> {
    let temp = unsafe {
        rw_txn
            .rw_txn_mut()
            .create_db(Some("temp_table"), DatabaseFlags::empty())
    }?;
    copy_table(env, rw_txn, db, temp)?;
    crate::Transaction::refresh(rw_txn);
    rw_txn.rw_txn_mut().clear_db(db)?;
    copy_table(env, rw_txn, temp, db)?;
    unsafe { rw_txn.rw_txn_mut().drop_db(temp) }?;
    crate::Transaction::refresh(rw_txn);
    Ok(())
}

fn copy_table<T: EnvironmentStrategy + 'static>(
    env: &LmdbEnv<T>,
    rw_txn: &mut LmdbWriteTransaction,
    source: Database,
    target: Database,
) -> anyhow::Result<()> {
    let ro_txn = env.tx_begin_read()?;
    {
        let mut cursor = ro_txn.txn().open_ro_cursor(source)?;
        for x in cursor.iter_start() {
            let (k, v) = x?;
            rw_txn
                .rw_txn_mut()
                .put(target, &k, &v, WriteFlags::APPEND)?;
        }
    }
    if ro_txn.txn().count(source) != rw_txn.rw_txn_mut().stat(target)?.entries() as u64 {
        bail!("table count mismatch");
    }
    Ok(())
}

fn do_upgrades(env: Arc<LmdbEnv>, logger: &dyn Logger) -> anyhow::Result<Vacuuming> {
    let version_store = LmdbVersionStore::new(env.clone())?;
    let mut txn = env.tx_begin_write()?;

    let version = match version_store.get(&txn) {
        Some(v) => v,
        None => {
            let new_version = STORE_VERSION_MINIMUM;
            logger.always_log(&format!("Setting db version to {}", new_version));
            version_store.put(&mut txn, new_version);
            new_version
        }
    };

    if version < 21 {
        logger.always_log(&format!("The version of the ledger ({}) is lower than the minimum ({}) which is supported for upgrades. Either upgrade to a v23 node first or delete the ledger.", version, STORE_VERSION_MINIMUM));
        return Err(anyhow!("version too low"));
    }

    if version > 22 {
        logger.always_log(&format!(
            "The version of the ledger ({}) is too high for this node",
            version
        ));
        return Err(anyhow!("version too high"));
    }

    if version == 21 {
        unsafe {
            let rw_txn = txn.rw_txn_mut();
            let db = rw_txn.create_db(Some("unchecked"), DatabaseFlags::empty())?;
            rw_txn.drop_db(db)?;
        }
    }

    // most recent version
    Ok(Vacuuming::NotNeeded)
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
fn copy_db<T: EnvironmentStrategy>(env: &LmdbEnv<T>, destination: &Path) -> anyhow::Result<()> {
    let c_path = CString::new(destination.as_os_str().to_str().unwrap()).unwrap();
    let status =
        unsafe { lmdb_sys::mdb_env_copy2(env.environment.env(), c_path.as_ptr(), MDB_CP_COMPACT) };
    ensure_success(status)
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
    let source_path = env.file_path()?;

    let backup_path = backup_file_path(&source_path)?;

    let start_message = format!(
        "Performing {:?} backup before database upgrade...",
        source_path
    );
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
        let error_message = format!("{:?} backup failed", source_path);
        logger.always_log(&error_message);
        eprintln!("{}", error_message);
        Err(anyhow!(error_message))
    } else {
        let success_message = format!("Backup created: {:?}", backup_path);
        logger.always_log(&success_message);
        println!("{}", success_message);
        Ok(())
    }
}

fn backup_file_path(source_path: &Path) -> anyhow::Result<PathBuf> {
    let extension = source_path
        .extension()
        .ok_or_else(|| anyhow!("no extension"))?
        .to_str()
        .ok_or_else(|| anyhow!("invalid extension"))?;

    let mut backup_path = source_path
        .parent()
        .ok_or_else(|| anyhow!("no parent path"))?
        .to_owned();

    let file_stem = source_path
        .file_stem()
        .ok_or_else(|| anyhow!("no file stem"))?
        .to_str()
        .ok_or_else(|| anyhow!("invalid file stem"))?;

    let backup_filename = format!(
        "{}_backup_{}.{}",
        file_stem,
        seconds_since_epoch(),
        extension
    );
    backup_path.push(&backup_filename);
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestDbFile;

    #[test]
    fn create_store() -> anyhow::Result<()> {
        let file = TestDbFile::random();
        let _ = LmdbStore::<EnvironmentWrapper>::open(&file.path).build()?;
        Ok(())
    }

    #[test]
    fn version_too_high_for_upgrade() -> anyhow::Result<()> {
        let file = TestDbFile::random();
        set_store_version(&file, i32::MAX)?;
        assert_upgrade_fails(&file.path, "version too high");
        Ok(())
    }

    #[test]
    fn version_too_low_for_upgrade() -> anyhow::Result<()> {
        let file = TestDbFile::random();
        set_store_version(&file, STORE_VERSION_MINIMUM - 1)?;
        assert_upgrade_fails(&file.path, "version too low");
        Ok(())
    }

    #[test]
    fn writes_db_version_for_new_store() {
        let file = TestDbFile::random();
        let store = LmdbStore::<EnvironmentWrapper>::open(&file.path)
            .build()
            .unwrap();
        let txn = store.tx_begin_read();
        assert_eq!(store.version.get(&txn), Some(STORE_VERSION_MINIMUM));
    }

    fn assert_upgrade_fails(path: &Path, error_msg: &str) {
        match LmdbStore::<EnvironmentWrapper>::open(path).build() {
            Ok(_) => panic!("store should not be created!"),
            Err(e) => {
                assert_eq!(e.to_string(), error_msg);
            }
        }
    }

    fn set_store_version(file: &TestDbFile, current_version: i32) -> Result<(), anyhow::Error> {
        let env = Arc::new(LmdbEnv::<EnvironmentWrapper>::new(&file.path)?);
        let version_store = LmdbVersionStore::new(env.clone())?;
        let mut txn = env.tx_begin_write()?;
        version_store.put(&mut txn, current_version);
        Ok(())
    }
}
