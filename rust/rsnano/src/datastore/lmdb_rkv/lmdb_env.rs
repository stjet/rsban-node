use lmdb::{Environment, EnvironmentFlags};
use std::{
    fs::{create_dir_all, set_permissions, Permissions},
    os::unix::prelude::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    datastore::lmdb::{NullTxnCallbacks, TxnCallbacks, TxnTracker},
    logger_mt::Logger,
    memory_intensive_instrumentation,
    utils::PropertyTreeWriter,
    LmdbConfig, SyncStrategy, TxnTrackingConfig,
};

use super::{LmdbReadTransaction, LmdbWriteTransaction};

#[derive(Default)]
pub struct EnvOptions {
    pub config: LmdbConfig,
    pub use_no_mem_init: bool,
}

pub struct LmdbEnv {
    pub environment: Environment,
    next_txn_id: AtomicU64,
    txn_tracker: Option<Arc<TxnTracker>>,
}

impl LmdbEnv {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::with_options(path, &EnvOptions::default())
    }

    pub fn with_options(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<Self> {
        let env = Self {
            environment: Self::init(path, options)?,
            next_txn_id: AtomicU64::new(0),
            txn_tracker: None,
        };
        Ok(env)
    }

    pub fn with_tracking(
        path: &Path,
        options: &EnvOptions,
        tracking_cfg: TxnTrackingConfig,
        block_processor_batch_max_time: Duration,
        logger: Arc<dyn Logger>,
    ) -> anyhow::Result<Self> {
        let txn_tracker = if tracking_cfg.enable {
            Some(Arc::new(TxnTracker::new(
                logger,
                tracking_cfg,
                block_processor_batch_max_time,
            )))
        } else {
            None
        };

        let result = Self {
            environment: Self::init(path, options)?,
            next_txn_id: AtomicU64::new(0),
            txn_tracker,
        };
        Ok(result)
    }

    pub fn init(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<Environment> {
        let path = path.as_ref();
        let parent = path.parent().ok_or_else(|| anyhow!("no parent path"))?;
        create_dir_all(parent)?;
        let perms = Permissions::from_mode(0o700);
        set_permissions(parent, perms)?;
        let mut map_size = options.config.map_size;
        let max_instrumented_map_size = 16 * 1024 * 1024;
        if memory_intensive_instrumentation() && map_size > max_instrumented_map_size {
            // In order to run LMDB under Valgrind, the maximum map size must be smaller than half your available RAM
            map_size = max_instrumented_map_size;
        }

        // It seems if there's ever more threads than mdb_env_set_maxreaders has read slots available, we get failures on transaction creation unless MDB_NOTLS is specified
        // This can happen if something like 256 io_threads are specified in the node config
        // MDB_NORDAHEAD will allow platforms that support it to load the DB in memory as needed.
        // MDB_NOMEMINIT prevents zeroing malloc'ed pages. Can provide improvement for non-sensitive data but may make memory checkers noisy (e.g valgrind).
        let mut environment_flags = EnvironmentFlags::NO_SUB_DIR
            | EnvironmentFlags::NO_TLS
            | EnvironmentFlags::NO_READAHEAD;
        if options.config.sync == SyncStrategy::NosyncSafe {
            environment_flags |= EnvironmentFlags::NO_META_SYNC;
        } else if options.config.sync == SyncStrategy::NosyncUnsafe {
            environment_flags |= EnvironmentFlags::NO_SYNC;
        } else if options.config.sync == SyncStrategy::NosyncUnsafeLargeMemory {
            environment_flags |= EnvironmentFlags::NO_SYNC
                | EnvironmentFlags::WRITE_MAP
                | EnvironmentFlags::MAP_ASYNC;
        }

        if !memory_intensive_instrumentation() && options.use_no_mem_init {
            environment_flags |= EnvironmentFlags::NO_MEM_INIT;
        }

        let env = Environment::new()
            .set_max_dbs(options.config.max_databases)
            .set_map_size(map_size)
            .set_flags(environment_flags)
            .open_with_permissions(path, 0o600)?;
        Ok(env)
    }

    pub fn tx_begin_read(&self) -> lmdb::Result<LmdbReadTransaction> {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbReadTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
    }

    pub fn tx_begin_write(&self) -> lmdb::Result<LmdbWriteTransaction> {
        // For IO threads, we do not want them to block on creating write transactions.
        debug_assert!(std::thread::current().name() != Some("I/O"));
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbWriteTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
    }

    fn create_txn_callbacks(&self) -> Arc<dyn TxnCallbacks> {
        match &self.txn_tracker {
            Some(tracker) => Arc::clone(tracker) as Arc<dyn TxnCallbacks>,
            None => Arc::new(NullTxnCallbacks::new()),
        }
    }

    pub fn serialize_txn_tracker(
        &self,
        json: &mut dyn PropertyTreeWriter,
        min_read_time: Duration,
        min_write_time: Duration,
    ) -> anyhow::Result<()> {
        match &self.txn_tracker {
            Some(tracker) => tracker.serialize_json(json, min_read_time, min_write_time),
            None => Ok(()),
        }
    }
}

impl Drop for LmdbEnv {
    fn drop(&mut self) {
        let _ = self.environment.sync(true);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    struct TestDbFile {
        pub path: PathBuf,
    }

    impl TestDbFile {
        fn new(path: impl AsRef<Path>) -> Self {
            Self {
                path: Path::new("/tmp").join(path),
            }
        }
    }

    impl Drop for TestDbFile {
        fn drop(&mut self) {
            if self.path.exists() {
                std::fs::remove_file(&self.path).unwrap();
                if let Some(parent) = self.path.parent() {
                    if parent != Path::new("/tmp") {
                        std::fs::remove_dir(parent).unwrap();
                    }
                }
            }
        }
    }

    mod test_db_file {
        use super::*;

        #[test]
        fn dont_panic_when_file_not_found() {
            let file = TestDbFile::new("does-not-exist.ldb");
            drop(file)
        }

        #[test]
        fn delete_file_when_dropped() {
            let file = TestDbFile::new("drop-test.ldb");
            std::fs::write(&file.path, "foo").unwrap();
            let path = file.path.clone();
            drop(file);
            assert_eq!(path.exists(), false);
        }

        #[test]
        fn delete_dir_when_dropped() {
            let file = TestDbFile::new("drop-dir/db.ldb");
            std::fs::create_dir(file.path.parent().unwrap()).unwrap();
            std::fs::write(&file.path, "foo").unwrap();
            let path = file.path.clone();
            drop(file);
            assert_eq!(path.exists(), false);
            assert_eq!(path.parent().unwrap().exists(), false);
        }
    }

    #[test]
    #[ignore]
    fn first_test() {
        let db_file = TestDbFile::new("foo.ldb");
        let env = LmdbEnv::new(&db_file.path).unwrap();
        let mut txn = env.tx_begin_read().unwrap();
        txn.refresh();
        assert!(true)
    }
}
