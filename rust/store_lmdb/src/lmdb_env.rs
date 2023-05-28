use crate::{
    LmdbConfig, LmdbReadTransaction, LmdbWriteTransaction, NullTransactionTracker, SyncStrategy,
    TransactionTracker,
};
use anyhow::bail;
use lmdb::{DatabaseFlags, EnvironmentFlags, Stat, Transaction};
use lmdb_sys::{MDB_env, MDB_SUCCESS};
use rsnano_core::utils::{memory_intensive_instrumentation, PropertyTreeWriter};
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::PathBuf;
use std::{
    ffi::{c_char, CStr},
    fs::{create_dir_all, set_permissions, Permissions},
    os::unix::prelude::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

// Thin Wrappers + Embedded Stubs
// --------------------------------------------------------------------------------

pub trait RwTransaction2<'env> {
    type Database;
    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]>;
    fn put(
        &mut self,
        database: Self::Database,
        key: &[u8],
        data: &[u8],
        flags: lmdb::WriteFlags,
    ) -> lmdb::Result<()>;

    fn del(
        &mut self,
        database: Self::Database,
        key: &[u8],
        flags: Option<&[u8]>,
    ) -> lmdb::Result<()>;

    unsafe fn create_db(
        &self,
        name: Option<&str>,
        flags: DatabaseFlags,
    ) -> lmdb::Result<Self::Database>;
    unsafe fn drop_db(&mut self, database: Self::Database) -> lmdb::Result<()>;
    fn clear_db(&mut self, database: Self::Database) -> lmdb::Result<()>;
    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<lmdb::RoCursor>;
    fn count(&self, database: Self::Database) -> u64;
    fn commit(self) -> lmdb::Result<()>;
}

pub struct RwTransactionWrapper<'env>(lmdb::RwTransaction<'env>);

impl<'env> RwTransaction2<'env> for RwTransactionWrapper<'env> {
    type Database = lmdb::Database;

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(&self.0, database, &&*key)
    }

    fn put(
        &mut self,
        database: Self::Database,
        key: &[u8],
        data: &[u8],
        flags: lmdb::WriteFlags,
    ) -> lmdb::Result<()> {
        lmdb::RwTransaction::put(&mut self.0, database, &&*key, &&*data, flags)
    }

    fn del(
        &mut self,
        database: Self::Database,
        key: &[u8],
        flags: Option<&[u8]>,
    ) -> lmdb::Result<()> {
        lmdb::RwTransaction::del(&mut self.0, database, &&*key, flags)
    }

    fn clear_db(&mut self, database: Self::Database) -> lmdb::Result<()> {
        lmdb::RwTransaction::clear_db(&mut self.0, database)
    }

    fn commit(self) -> lmdb::Result<()> {
        self.0.commit()
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<lmdb::RoCursor> {
        lmdb::Transaction::open_ro_cursor(&self.0, database)
    }

    fn count(&self, database: Self::Database) -> u64 {
        let stat = lmdb::Transaction::stat(&self.0, database);
        stat.unwrap().entries() as u64
    }

    unsafe fn drop_db(&mut self, database: Self::Database) -> lmdb::Result<()> {
        lmdb::RwTransaction::drop_db(&mut self.0, database)
    }

    unsafe fn create_db(
        &self,
        name: Option<&str>,
        flags: DatabaseFlags,
    ) -> lmdb::Result<Self::Database> {
        lmdb::RwTransaction::create_db(&self.0, name, flags)
    }
}

pub struct NullRwTransaction<'env>(PhantomData<&'env ()>);

impl<'env> RwTransaction2<'env> for NullRwTransaction<'env> {
    type Database = DatabaseStub;

    fn get(&self, _database: Self::Database, _key: &[u8]) -> lmdb::Result<&[u8]> {
        Err(lmdb::Error::NotFound)
    }

    fn put(
        &mut self,
        _database: Self::Database,
        _key: &[u8],
        _data: &[u8],
        _flags: lmdb::WriteFlags,
    ) -> lmdb::Result<()> {
        Ok(())
    }

    fn del(
        &mut self,
        _database: Self::Database,
        _key: &[u8],
        _flags: Option<&[u8]>,
    ) -> lmdb::Result<()> {
        Ok(())
    }

    fn clear_db(&mut self, _database: Self::Database) -> lmdb::Result<()> {
        Ok(())
    }

    fn commit(self) -> lmdb::Result<()> {
        Ok(())
    }

    fn open_ro_cursor(&self, _database: Self::Database) -> lmdb::Result<lmdb::RoCursor> {
        Err(lmdb::Error::NotFound)
    }

    fn count(&self, _database: Self::Database) -> u64 {
        0
    }

    unsafe fn drop_db(&mut self, _database: Self::Database) -> lmdb::Result<()> {
        Ok(())
    }

    unsafe fn create_db(
        &self,
        _name: Option<&str>,
        _flags: DatabaseFlags,
    ) -> lmdb::Result<Self::Database> {
        Ok(DatabaseStub(42))
    }
}

pub trait InactiveTransaction<'env> {
    type RoTxnType: RoTransaction<'env>;
    fn renew(self) -> lmdb::Result<Self::RoTxnType>;
}

pub trait RoTransaction<'env> {
    type InactiveTxnType: InactiveTransaction<'env, RoTxnType = Self>
    where
        Self: Sized;

    type Database;

    fn reset(self) -> Self::InactiveTxnType
    where
        Self: Sized;

    fn commit(self) -> lmdb::Result<()>;
    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]>;
    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<lmdb::RoCursor>;
    fn count(&self, database: Self::Database) -> u64;
}

pub struct InactiveTransactionWrapper<'env> {
    inactive: lmdb::InactiveTransaction<'env>,
    _marker: PhantomData<&'env ()>,
}

impl<'env> InactiveTransaction<'env> for InactiveTransactionWrapper<'env> {
    type RoTxnType = RoTransactionWrapper<'env>;
    fn renew(self) -> lmdb::Result<Self::RoTxnType> {
        self.inactive.renew().map(|t| RoTransactionWrapper(t))
    }
}

pub struct RoTransactionWrapper<'env>(lmdb::RoTransaction<'env>);

impl<'env> RoTransaction<'env> for RoTransactionWrapper<'env> {
    type InactiveTxnType = InactiveTransactionWrapper<'env>;
    type Database = lmdb::Database;

    fn reset(self) -> Self::InactiveTxnType {
        InactiveTransactionWrapper {
            inactive: self.0.reset(),
            _marker: Default::default(),
        }
    }

    fn commit(self) -> lmdb::Result<()> {
        lmdb::Transaction::commit(self.0)
    }

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(&self.0, database, &&*key)
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<lmdb::RoCursor> {
        lmdb::Transaction::open_ro_cursor(&self.0, database)
    }

    fn count(&self, database: Self::Database) -> u64 {
        let stat = lmdb::Transaction::stat(&self.0, database);
        stat.unwrap().entries() as u64
    }
}
pub struct NullRoTransaction<'env>(PhantomData<&'env ()>);
pub struct NullInactiveTransaction<'env>(PhantomData<&'env ()>);

impl<'env> RoTransaction<'env> for NullRoTransaction<'env> {
    type InactiveTxnType = NullInactiveTransaction<'env>;
    type Database = DatabaseStub;

    fn reset(self) -> Self::InactiveTxnType
    where
        Self: Sized,
    {
        NullInactiveTransaction(Default::default())
    }

    fn commit(self) -> lmdb::Result<()> {
        Ok(())
    }

    fn get(&self, _database: Self::Database, _key: &[u8]) -> lmdb::Result<&[u8]> {
        Err(lmdb::Error::NotFound)
    }

    fn open_ro_cursor(&self, _database: Self::Database) -> lmdb::Result<lmdb::RoCursor> {
        Err(lmdb::Error::NotFound)
    }

    fn count(&self, _database: Self::Database) -> u64 {
        0
    }
}

impl<'env> InactiveTransaction<'env> for NullInactiveTransaction<'env> {
    type RoTxnType = NullRoTransaction<'env>;

    fn renew(self) -> lmdb::Result<Self::RoTxnType> {
        Ok(NullRoTransaction(Default::default()))
    }
}

pub struct EnvironmentOptions<'a> {
    pub max_dbs: u32,
    pub map_size: usize,
    pub flags: EnvironmentFlags,
    pub path: &'a Path,
    pub file_mode: u32,
}

pub trait Environment: Send + Sync {
    type RoTxnImpl<'env>: RoTransaction<
        'env,
        InactiveTxnType = Self::InactiveTxnImpl<'env>,
        Database = Self::Database,
    >
    where
        Self: 'env;

    type InactiveTxnImpl<'env>: InactiveTransaction<'env, RoTxnType = Self::RoTxnImpl<'env>>
    where
        Self: 'env;

    type RwTxnType<'env>: RwTransaction2<'env, Database = Self::Database>
    where
        Self: 'env;

    type Database: Send + Sync + Copy;

    fn build(options: EnvironmentOptions) -> lmdb::Result<Self>
    where
        Self: Sized;
    fn begin_ro_txn<'env>(&'env self) -> lmdb::Result<Self::RoTxnImpl<'env>>;
    fn begin_rw_txn<'env>(&'env self) -> lmdb::Result<Self::RwTxnType<'env>>;
    fn create_db<'env>(
        &'env self,
        name: Option<&str>,
        flags: DatabaseFlags,
    ) -> lmdb::Result<Self::Database>;

    fn env(&self) -> *mut MDB_env;
    fn open_db<'env>(&'env self, name: Option<&str>) -> lmdb::Result<Self::Database>;
    fn sync(&self, force: bool) -> lmdb::Result<()>;
    fn stat(&self) -> lmdb::Result<Stat>;
}

pub struct EnvironmentWrapper(lmdb::Environment);

impl Environment for EnvironmentWrapper {
    type RoTxnImpl<'env> = RoTransactionWrapper<'env>;
    type InactiveTxnImpl<'env> = InactiveTransactionWrapper<'env>;
    type RwTxnType<'env> = RwTransactionWrapper<'env>;
    type Database = lmdb::Database;

    fn build(options: EnvironmentOptions) -> lmdb::Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(options.max_dbs)
            .set_map_size(options.map_size)
            .set_flags(options.flags)
            .open_with_permissions(options.path, options.file_mode)?;
        Ok(Self(env))
    }

    fn begin_ro_txn<'env>(&'env self) -> lmdb::Result<Self::RoTxnImpl<'env>> {
        self.0.begin_ro_txn().map(|txn| RoTransactionWrapper(txn))
    }

    fn begin_rw_txn<'env>(&'env self) -> lmdb::Result<Self::RwTxnType<'env>> {
        self.0.begin_rw_txn().map(|txn| RwTransactionWrapper(txn))
    }

    fn create_db<'env>(
        &'env self,
        name: Option<&str>,
        flags: DatabaseFlags,
    ) -> lmdb::Result<lmdb::Database> {
        self.0.create_db(name, flags)
    }

    fn env(&self) -> *mut MDB_env {
        self.0.env()
    }

    fn open_db<'env>(&'env self, name: Option<&str>) -> lmdb::Result<lmdb::Database> {
        self.0.open_db(name)
    }

    fn sync(&self, force: bool) -> lmdb::Result<()> {
        self.0.sync(force)
    }

    fn stat(&self) -> lmdb::Result<Stat> {
        self.0.stat()
    }
}

pub struct EnvironmentStub;
#[derive(Clone, Copy)]
pub struct DatabaseStub(u32);

impl Environment for EnvironmentStub {
    type RoTxnImpl<'env> = NullRoTransaction<'env>;
    type InactiveTxnImpl<'env> = NullInactiveTransaction<'env>;
    type RwTxnType<'env> = NullRwTransaction<'env>;
    type Database = DatabaseStub;

    fn build(_options: EnvironmentOptions) -> lmdb::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn begin_ro_txn<'env>(&'env self) -> lmdb::Result<Self::RoTxnImpl<'env>> {
        Ok(NullRoTransaction(Default::default()))
    }

    fn begin_rw_txn<'env>(&'env self) -> lmdb::Result<Self::RwTxnType<'env>> {
        Ok(NullRwTransaction(Default::default()))
    }

    fn create_db<'env>(
        &'env self,
        _name: Option<&str>,
        _flags: DatabaseFlags,
    ) -> lmdb::Result<Self::Database> {
        Ok(DatabaseStub(42))
    }

    fn env(&self) -> *mut MDB_env {
        todo!()
    }

    fn open_db<'env>(&'env self, _name: Option<&str>) -> lmdb::Result<Self::Database> {
        todo!()
    }

    fn sync(&self, _force: bool) -> lmdb::Result<()> {
        Ok(())
    }

    fn stat(&self) -> lmdb::Result<Stat> {
        todo!()
    }
}

// Environment
// --------------------------------------------------------------------------------

#[derive(Default)]
pub struct EnvOptions {
    pub config: LmdbConfig,
    pub use_no_mem_init: bool,
}

pub struct LmdbEnv<T: Environment = EnvironmentWrapper> {
    pub environment: T,
    next_txn_id: AtomicU64,
    txn_tracker: Arc<dyn TransactionTracker>,
}

impl LmdbEnv<EnvironmentStub> {
    pub fn create_null() -> Self {
        Self::new("nulled_data.ldb").unwrap()
    }
}

impl<T: Environment> LmdbEnv<T> {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::with_options(path, &EnvOptions::default())
    }

    pub fn with_options(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<Self> {
        let env = Self {
            environment: Self::init(path, options)?,
            next_txn_id: AtomicU64::new(0),
            txn_tracker: Arc::new(NullTransactionTracker::new()),
        };
        Ok(env)
    }

    pub fn with_txn_tracker(
        path: &Path,
        options: &EnvOptions,
        txn_tracker: Arc<dyn TransactionTracker>,
    ) -> anyhow::Result<Self> {
        let env = Self {
            environment: Self::init(path, options)?,
            next_txn_id: AtomicU64::new(0),
            txn_tracker,
        };
        Ok(env)
    }

    pub fn init(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<T> {
        let path = path.as_ref();
        try_create_parent_dir(path)?;
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

        let env = T::build(EnvironmentOptions {
            max_dbs: options.config.max_databases,
            map_size,
            flags: environment_flags,
            path,
            file_mode: 0o600,
        })?;
        Ok(env)
    }

    pub fn tx_begin_read(&self) -> lmdb::Result<LmdbReadTransaction<T>> {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbReadTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
    }

    pub fn tx_begin_write(&self) -> lmdb::Result<LmdbWriteTransaction<T>> {
        // For IO threads, we do not want them to block on creating write transactions.
        debug_assert!(std::thread::current().name() != Some("I/O"));
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbWriteTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
    }

    pub fn file_path(&self) -> anyhow::Result<PathBuf> {
        let mut path: *const c_char = std::ptr::null();
        let status = unsafe { lmdb_sys::mdb_env_get_path(self.environment.env(), &mut path) };
        if status != MDB_SUCCESS {
            bail!("could not get env path");
        }
        let source_path: PathBuf = unsafe { CStr::from_ptr(path) }.to_str()?.into();
        Ok(source_path)
    }

    fn create_txn_callbacks(&self) -> Arc<dyn TransactionTracker> {
        Arc::clone(&self.txn_tracker)
    }

    pub fn serialize_txn_tracker(
        &self,
        json: &mut dyn PropertyTreeWriter,
        min_read_time: Duration,
        min_write_time: Duration,
    ) -> anyhow::Result<()> {
        self.txn_tracker
            .serialize_json(json, min_read_time, min_write_time)
    }
}

fn try_create_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if parent != Path::new("") {
            if !parent.is_dir() {
                create_dir_all(parent)?;
                set_permissions(parent, Permissions::from_mode(0o700))?;
            }
        }
    }
    Ok(())
}

impl<T: Environment> Drop for LmdbEnv<T> {
    fn drop(&mut self) {
        let _ = self.environment.sync(true);
    }
}

pub struct TestDbFile {
    pub path: PathBuf,
}

impl TestDbFile {
    fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: Path::new("/tmp").join(path),
        }
    }

    pub fn random() -> Self {
        Self::new(Self::temp_file_name())
    }

    fn temp_file_name() -> PathBuf {
        PathBuf::from(format!("{}.ldb", uuid::Uuid::new_v4().to_simple()))
    }

    fn lock_file_path(&self) -> PathBuf {
        let mut lock_file_path = self.path.parent().unwrap().to_owned();
        let mut fname = self.path.file_name().unwrap().to_os_string();
        fname.push("-lock");
        lock_file_path.push(fname);
        lock_file_path
    }
}

impl Drop for TestDbFile {
    fn drop(&mut self) {
        if self.path.exists() {
            std::fs::remove_file(&self.path).unwrap();
            let lock_file = self.lock_file_path();
            if lock_file.exists() {
                std::fs::remove_file(&lock_file).unwrap();
            }

            if let Some(parent) = self.path.parent() {
                if parent != Path::new("/tmp") {
                    std::fs::remove_dir(parent).unwrap();
                }
            }
        }
    }
}

pub struct TestLmdbEnv {
    env: Arc<LmdbEnv>,
    _file: TestDbFile,
}

impl TestLmdbEnv {
    pub fn new() -> Self {
        let file = TestDbFile::random();
        let env = Arc::new(LmdbEnv::new(&file.path).unwrap());
        Self { _file: file, env }
    }

    pub fn env(&self) -> Arc<LmdbEnv> {
        self.env.clone()
    }
}

impl Deref for TestLmdbEnv {
    type Target = LmdbEnv;

    fn deref(&self) -> &Self::Target {
        &self.env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod test_db_file {
        use super::*;

        #[test]
        fn tmp_test() {
            let path = Path::new("foo.tmp");
            assert_eq!(path.parent(), Some(Path::new("")));
            assert_eq!(Path::new(""), Path::new(""))
        }

        #[test]
        fn dont_panic_when_file_not_found() {
            let file = TestDbFile::new("does-not-exist.ldb");
            drop(file)
        }

        #[test]
        fn delete_file_when_dropped() {
            let file = TestDbFile::new("drop-test.ldb");
            let mut lock_file_path = file.path.parent().unwrap().to_owned();
            lock_file_path.push("drop-test.ldb-lock");
            std::fs::write(&file.path, "foo").unwrap();
            std::fs::write(&lock_file_path, "foo").unwrap();
            let path = file.path.clone();
            drop(file);
            assert_eq!(path.exists(), false, "db file was not deleted");
            assert_eq!(lock_file_path.exists(), false, "lock file was not deleted");
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

        #[test]
        fn tmp_file_name() {
            let filename = TestDbFile::temp_file_name();
            assert_eq!(filename.extension().unwrap(), "ldb");
            assert_eq!(filename.file_stem().unwrap().len(), 32);
            assert_ne!(TestDbFile::temp_file_name(), TestDbFile::temp_file_name());
        }
    }
}
