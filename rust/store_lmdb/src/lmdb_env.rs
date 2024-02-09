use crate::{
    LmdbConfig, LmdbReadTransaction, LmdbWriteTransaction, NullTransactionTracker, SyncStrategy,
    TransactionTracker,
};
use anyhow::bail;
use lmdb::{DatabaseFlags, EnvironmentFlags, Stat, Transaction};
use lmdb_sys::{MDB_env, MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE, MDB_SUCCESS};
use rsnano_core::utils::{memory_intensive_instrumentation, PropertyTreeWriter};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
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
use tracing::debug;

// Thin Wrappers + Embedded Stubs
// --------------------------------------------------------------------------------

//todo don't use static lifetimes!
pub trait RoCursor {
    type Iter: Iterator<Item = lmdb::Result<(&'static [u8], &'static [u8])>>;
    fn iter_start(&mut self) -> Self::Iter;
    fn get(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])>;
}

//todo don't use static lifetimes!
pub struct RoCursorWrapper(lmdb::RoCursor<'static>);

impl RoCursor for RoCursorWrapper {
    type Iter = lmdb::Iter<'static>;
    fn iter_start(&mut self) -> lmdb::Iter<'static> {
        lmdb::Cursor::iter_start(&mut self.0)
    }

    fn get(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])> {
        lmdb::Cursor::get(&self.0, key, data, op)
    }
}

pub struct RoCursorStub {
    database: ConfiguredDatabase,
    current: Cell<i32>,
    ascending: Cell<bool>,
}

pub struct NullIter {}

impl Iterator for NullIter {
    type Item = lmdb::Result<(&'static [u8], &'static [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

impl RoCursor for RoCursorStub {
    type Iter = NullIter;

    fn iter_start(&mut self) -> NullIter {
        NullIter {}
    }

    fn get(
        &self,
        key: Option<&[u8]>,
        _data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])> {
        if op == MDB_FIRST {
            self.current.set(0);
            self.ascending.set(true);
        } else if op == MDB_LAST {
            let entry_count = self.database.entries.len();
            self.ascending.set(false);
            self.current.set((entry_count as i32) - 1);
        } else if op == MDB_NEXT {
            if self.ascending.get() {
                self.current.set(self.current.get() + 1);
            } else {
                self.current.set(self.current.get() - 1);
            }
        } else if op == MDB_SET_RANGE {
            self.current.set(
                self.database
                    .entries
                    .keys()
                    .enumerate()
                    .find_map(|(i, k)| {
                        if Some(k.as_slice()) >= key {
                            Some(i as i32)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(i32::MAX),
            );
        } else {
            unimplemented!()
        }

        let current = self.current.get();
        if current < 0 {
            return Err(lmdb::Error::NotFound);
        }

        self.database
            .entries
            .iter()
            .nth(current as usize)
            .map(|(k, v)| unsafe {
                (
                    Some(std::mem::transmute::<&'_ [u8], &'static [u8]>(k.as_slice())),
                    std::mem::transmute::<&'_ [u8], &'static [u8]>(v.as_slice()),
                )
            })
            .ok_or(lmdb::Error::NotFound)
    }
}

pub trait RwTransaction {
    type Database;
    type RoCursor: RoCursor;
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
    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor>;
    fn count(&self, database: Self::Database) -> u64;
    fn commit(self) -> lmdb::Result<()>;
}

pub struct RwTransactionWrapper(lmdb::RwTransaction<'static>);

impl RwTransaction for RwTransactionWrapper {
    type Database = lmdb::Database;
    type RoCursor = RoCursorWrapper;

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(&self.0, database, &key)
    }

    fn put(
        &mut self,
        database: Self::Database,
        key: &[u8],
        data: &[u8],
        flags: lmdb::WriteFlags,
    ) -> lmdb::Result<()> {
        lmdb::RwTransaction::put(&mut self.0, database, &key, &data, flags)
    }

    fn del(
        &mut self,
        database: Self::Database,
        key: &[u8],
        flags: Option<&[u8]>,
    ) -> lmdb::Result<()> {
        lmdb::RwTransaction::del(&mut self.0, database, &key, flags)
    }

    fn clear_db(&mut self, database: Self::Database) -> lmdb::Result<()> {
        lmdb::RwTransaction::clear_db(&mut self.0, database)
    }

    fn commit(self) -> lmdb::Result<()> {
        self.0.commit()
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        let cursor = lmdb::Transaction::open_ro_cursor(&self.0, database);
        cursor.map(|c| {
            // todo: don't use static lifetime
            let c =
                unsafe { std::mem::transmute::<lmdb::RoCursor<'_>, lmdb::RoCursor<'static>>(c) };
            RoCursorWrapper(c)
        })
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

pub struct RwTransactionStub {
    databases: Vec<ConfiguredDatabase>,
}

impl RwTransactionStub {
    fn get_database(&self, database: DatabaseStub) -> Option<&ConfiguredDatabase> {
        self.databases.iter().find(|d| d.dbi == database)
    }
}

impl RwTransaction for RwTransactionStub {
    type Database = DatabaseStub;
    type RoCursor = RoCursorStub;

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        let Some(db) = self.get_database(database) else {
            return Err(lmdb::Error::NotFound);
        };
        match db.entries.get(key) {
            Some(value) => Ok(value),
            None => Err(lmdb::Error::NotFound),
        }
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

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        Ok(RoCursorStub {
            current: Cell::new(0),
            ascending: Cell::new(true),
            database: self
                .databases
                .iter()
                .find(|db| db.dbi == database)
                .cloned()
                .unwrap_or_default(),
        })
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

pub trait InactiveTransaction {
    type RoTxnType: RoTransaction;
    fn renew(self) -> lmdb::Result<Self::RoTxnType>;
}

pub trait RoTransaction {
    type InactiveTxnType: InactiveTransaction<RoTxnType = Self>
    where
        Self: Sized;

    type Database;
    type RoCursor: RoCursor;

    fn reset(self) -> Self::InactiveTxnType
    where
        Self: Sized;

    fn commit(self) -> lmdb::Result<()>;
    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]>;
    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor>;
    fn count(&self, database: Self::Database) -> u64;
}

pub struct InactiveTransactionWrapper {
    inactive: lmdb::InactiveTransaction<'static>,
}

impl InactiveTransaction for InactiveTransactionWrapper {
    type RoTxnType = RoTransactionWrapper;
    fn renew(self) -> lmdb::Result<Self::RoTxnType> {
        self.inactive.renew().map(RoTransactionWrapper)
    }
}

pub struct RoTransactionWrapper(lmdb::RoTransaction<'static>);

impl RoTransaction for RoTransactionWrapper {
    type InactiveTxnType = InactiveTransactionWrapper;
    type Database = lmdb::Database;
    type RoCursor = RoCursorWrapper;

    fn reset(self) -> Self::InactiveTxnType {
        InactiveTransactionWrapper {
            inactive: self.0.reset(),
        }
    }

    fn commit(self) -> lmdb::Result<()> {
        lmdb::Transaction::commit(self.0)
    }

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(&self.0, database, &&*key)
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        lmdb::Transaction::open_ro_cursor(&self.0, database).map(|c| {
            //todo don't use static lifetime
            let c =
                unsafe { std::mem::transmute::<lmdb::RoCursor<'_>, lmdb::RoCursor<'static>>(c) };
            RoCursorWrapper(c)
        })
    }

    fn count(&self, database: Self::Database) -> u64 {
        let stat = lmdb::Transaction::stat(&self.0, database);
        stat.unwrap().entries() as u64
    }
}
pub struct RoTransactionStub {
    databases: Vec<ConfiguredDatabase>,
}

impl RoTransactionStub {
    fn get_database(&self, database: DatabaseStub) -> Option<&ConfiguredDatabase> {
        self.databases.iter().find(|d| d.dbi == database)
    }
}
pub struct NullInactiveTransaction {
    databases: Vec<ConfiguredDatabase>,
}

impl RoTransaction for RoTransactionStub {
    type InactiveTxnType = NullInactiveTransaction;
    type Database = DatabaseStub;
    type RoCursor = RoCursorStub;

    fn reset(self) -> Self::InactiveTxnType
    where
        Self: Sized,
    {
        NullInactiveTransaction {
            databases: self.databases,
        }
    }

    fn commit(self) -> lmdb::Result<()> {
        Ok(())
    }

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        let Some(db) = self.get_database(database) else {
            return Err(lmdb::Error::NotFound);
        };
        match db.entries.get(key) {
            Some(value) => Ok(value),
            None => Err(lmdb::Error::NotFound),
        }
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        match self.get_database(database) {
            Some(db) => Ok(RoCursorStub {
                current: Cell::new(0),
                database: db.clone(),
                ascending: Cell::new(true),
            }),
            None => Ok(RoCursorStub {
                current: Cell::new(0),
                ascending: Cell::new(true),
                database: ConfiguredDatabase {
                    dbi: database,
                    db_name: "test_database".to_string(),
                    entries: Default::default(),
                },
            }),
        }
    }

    fn count(&self, database: Self::Database) -> u64 {
        self.get_database(database)
            .map(|db| db.entries.len())
            .unwrap_or_default() as u64
    }
}

impl InactiveTransaction for NullInactiveTransaction {
    type RoTxnType = RoTransactionStub;

    fn renew(self) -> lmdb::Result<Self::RoTxnType> {
        Ok(RoTransactionStub {
            databases: self.databases,
        })
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
    type RoTxnImpl: RoTransaction<
        InactiveTxnType = Self::InactiveTxnImpl,
        Database = Self::Database,
        RoCursor = Self::RoCursor,
    >;

    type InactiveTxnImpl: InactiveTransaction<RoTxnType = Self::RoTxnImpl>;

    type RwTxnType: RwTransaction<Database = Self::Database, RoCursor = Self::RoCursor>;

    type Database: Send + Sync + Copy;
    type RoCursor: RoCursor;

    fn build(options: EnvironmentOptions) -> lmdb::Result<Self>
    where
        Self: Sized;
    fn begin_ro_txn(&self) -> lmdb::Result<Self::RoTxnImpl>;
    fn begin_rw_txn(&self) -> lmdb::Result<Self::RwTxnType>;
    fn create_db(&self, name: Option<&str>, flags: DatabaseFlags) -> lmdb::Result<Self::Database>;

    fn env(&self) -> *mut MDB_env;
    fn open_db(&self, name: Option<&str>) -> lmdb::Result<Self::Database>;
    fn sync(&self, force: bool) -> lmdb::Result<()>;
    fn stat(&self) -> lmdb::Result<Stat>;
}

pub struct EnvironmentWrapper(lmdb::Environment);

impl Environment for EnvironmentWrapper {
    type RoTxnImpl = RoTransactionWrapper;
    type InactiveTxnImpl = InactiveTransactionWrapper;
    type RwTxnType = RwTransactionWrapper;
    type Database = lmdb::Database;
    type RoCursor = RoCursorWrapper;

    fn build(options: EnvironmentOptions) -> lmdb::Result<Self> {
        let env = lmdb::Environment::new()
            .set_max_dbs(options.max_dbs)
            .set_map_size(options.map_size)
            .set_flags(options.flags)
            .open_with_permissions(options.path, options.file_mode.try_into().unwrap())?;
        Ok(Self(env))
    }

    fn begin_ro_txn(&self) -> lmdb::Result<Self::RoTxnImpl> {
        self.0.begin_ro_txn().map(|txn| {
            // todo: don't use static life time
            let txn = unsafe {
                std::mem::transmute::<lmdb::RoTransaction<'_>, lmdb::RoTransaction<'static>>(txn)
            };
            RoTransactionWrapper(txn)
        })
    }

    fn begin_rw_txn(&self) -> lmdb::Result<Self::RwTxnType> {
        self.0.begin_rw_txn().map(|txn| {
            // todo: don't use static life time
            let txn = unsafe {
                std::mem::transmute::<lmdb::RwTransaction<'_>, lmdb::RwTransaction<'static>>(txn)
            };
            RwTransactionWrapper(txn)
        })
    }

    fn create_db(&self, name: Option<&str>, flags: DatabaseFlags) -> lmdb::Result<lmdb::Database> {
        self.0.create_db(name, flags)
    }

    fn env(&self) -> *mut MDB_env {
        self.0.env()
    }

    fn open_db(&self, name: Option<&str>) -> lmdb::Result<lmdb::Database> {
        self.0.open_db(name)
    }

    fn sync(&self, force: bool) -> lmdb::Result<()> {
        self.0.sync(force)
    }

    fn stat(&self) -> lmdb::Result<Stat> {
        self.0.stat()
    }
}

pub struct EnvironmentStub {
    databases: Vec<ConfiguredDatabase>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DatabaseStub(pub u32);

impl Default for DatabaseStub {
    fn default() -> Self {
        Self(42)
    }
}

impl Environment for EnvironmentStub {
    type RoTxnImpl = RoTransactionStub;
    type InactiveTxnImpl = NullInactiveTransaction;
    type RwTxnType = RwTransactionStub;
    type Database = DatabaseStub;
    type RoCursor = RoCursorStub;

    fn build(_options: EnvironmentOptions) -> lmdb::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            databases: Vec::new(),
        })
    }

    fn begin_ro_txn(&self) -> lmdb::Result<Self::RoTxnImpl> {
        Ok(RoTransactionStub {
            databases: self.databases.clone(), //todo  don't clone!
        })
    }

    fn begin_rw_txn(&self) -> lmdb::Result<Self::RwTxnType> {
        Ok(RwTransactionStub {
            databases: self.databases.clone(), //todo  don't clone!
        })
    }

    fn create_db(&self, name: Option<&str>, _flags: DatabaseFlags) -> lmdb::Result<Self::Database> {
        Ok(self
            .databases
            .iter()
            .find(|x| name == Some(&x.db_name))
            .map(|x| x.dbi)
            .unwrap_or_default())
    }

    fn env(&self) -> *mut MDB_env {
        todo!()
    }

    fn open_db(&self, name: Option<&str>) -> lmdb::Result<Self::Database> {
        self.create_db(name, DatabaseFlags::empty())
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

#[derive(Default, Debug)]
pub struct EnvOptions {
    pub config: LmdbConfig,
    pub use_no_mem_init: bool,
}

pub struct NullLmdbEnvBuilder {
    databases: Vec<ConfiguredDatabase>,
}

impl NullLmdbEnvBuilder {
    pub fn database(self, name: impl Into<String>, dbi: DatabaseStub) -> NullDatabaseBuilder {
        NullDatabaseBuilder {
            data: ConfiguredDatabase {
                dbi,
                db_name: name.into(),
                entries: BTreeMap::new(),
            },
            env_builder: self,
        }
    }

    pub fn configured_database(mut self, db: ConfiguredDatabase) -> Self {
        if self
            .databases
            .iter()
            .any(|x| x.dbi == db.dbi || x.db_name == db.db_name)
        {
            panic!(
                "trying to duplicated database for {} / {}",
                db.dbi.0, db.db_name
            );
        }
        self.databases.push(db);
        self
    }

    pub fn build(self) -> LmdbEnv<EnvironmentStub> {
        let env = EnvironmentStub {
            databases: self.databases,
        };
        LmdbEnv::with_env(env)
    }
}

#[derive(Clone)]
pub struct ConfiguredDatabase {
    pub dbi: DatabaseStub,
    pub db_name: String,
    pub entries: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl ConfiguredDatabase {
    pub fn new(dbi: DatabaseStub, name: impl Into<String>) -> Self {
        Self {
            dbi,
            db_name: name.into(),
            entries: BTreeMap::new(),
        }
    }
}

impl Default for ConfiguredDatabase {
    fn default() -> Self {
        Self {
            dbi: DatabaseStub(42),
            db_name: "nulled_database".to_string(),
            entries: Default::default(),
        }
    }
}

pub struct NullDatabaseBuilder {
    env_builder: NullLmdbEnvBuilder,
    data: ConfiguredDatabase,
}

impl NullDatabaseBuilder {
    pub fn entry(mut self, key: &[u8], value: &[u8]) -> Self {
        self.data.entries.insert(key.to_vec(), value.to_vec());
        self
    }
    pub fn build(mut self) -> NullLmdbEnvBuilder {
        self.env_builder.databases.push(self.data);
        self.env_builder
    }
}

pub struct LmdbEnv<T: Environment = EnvironmentWrapper> {
    pub environment: T,
    next_txn_id: AtomicU64,
    txn_tracker: Arc<dyn TransactionTracker>,
    env_id: usize,
}

impl LmdbEnv<EnvironmentStub> {
    pub fn create_null() -> Self {
        Self::new("nulled_data.ldb").unwrap()
    }

    pub fn create_null_with() -> NullLmdbEnvBuilder {
        NullLmdbEnvBuilder {
            databases: Vec::new(),
        }
    }
}

static ENV_COUNT: AtomicUsize = AtomicUsize::new(0);
static NEXT_ENV_ID: AtomicUsize = AtomicUsize::new(0);

impl<T: Environment> LmdbEnv<T> {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::with_options(path, &EnvOptions::default())
    }

    pub fn with_env(env: T) -> Self {
        ENV_COUNT.fetch_add(1, Ordering::SeqCst);
        let env_id = NEXT_ENV_ID.fetch_add(1, Ordering::SeqCst);
        let alive = ENV_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        debug!(env_id, alive, "LMDB env created",);
        Self {
            environment: env,
            next_txn_id: AtomicU64::new(0),
            txn_tracker: Arc::new(NullTransactionTracker::new()),
            env_id,
        }
    }

    pub fn with_options(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let env = Self {
            environment: Self::init(path, options)?,
            next_txn_id: AtomicU64::new(0),
            txn_tracker: Arc::new(NullTransactionTracker::new()),
            env_id: NEXT_ENV_ID.fetch_add(1, Ordering::SeqCst),
        };
        let alive = ENV_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        debug!(env_id = env.env_id, alive, ?path, "LMDB env created",);
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
            env_id: NEXT_ENV_ID.fetch_add(1, Ordering::SeqCst),
        };
        let alive = ENV_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        debug!(env_id = env.env_id, alive, ?path, "LMDB env created",);
        Ok(env)
    }

    pub fn init(path: impl AsRef<Path>, options: &EnvOptions) -> anyhow::Result<T> {
        let path = path.as_ref();
        debug_assert!(
            path.extension() == Some(&OsStr::new("ldb")),
            "invalid filename extension for lmdb database file"
        );
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

    pub fn tx_begin_read(&self) -> LmdbReadTransaction<T> {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbReadTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
            .expect("Could not create LMDB read-only transaction")
    }

    pub fn tx_begin_write(&self) -> LmdbWriteTransaction<T> {
        // For IO threads, we do not want them to block on creating write transactions.
        debug_assert!(std::thread::current().name() != Some("I/O"));
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::Relaxed);
        LmdbWriteTransaction::new(txn_id, &self.environment, self.create_txn_callbacks())
            .expect("Could not create LMDB read-write transaction")
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
        if parent != Path::new("") && !parent.is_dir() {
            create_dir_all(parent)?;
            set_permissions(parent, Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

impl<T: Environment> Drop for LmdbEnv<T> {
    fn drop(&mut self) {
        let alive = ENV_COUNT.fetch_sub(1, Ordering::Relaxed) - 1;
        debug!(env_id = self.env_id, alive, "LMDB env dropped",);
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
    use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_SET_RANGE};

    use super::*;

    mod rw_txn {
        use lmdb::WriteFlags;

        use crate::PutEvent;

        use super::*;

        #[test]
        fn can_track_puts() {
            let env = LmdbEnv::create_null();
            let mut txn = env.tx_begin_write();
            let tracker = txn.track_puts();

            let database = DatabaseStub(42);
            let key = &[1, 2, 3];
            let value = &[4, 5, 6];
            let flags = WriteFlags::APPEND;
            txn.put(database, key, value, flags).unwrap();

            let puts = tracker.output();
            assert_eq!(
                puts,
                vec![PutEvent {
                    database,
                    key: key.to_vec(),
                    value: value.to_vec(),
                    flags
                }]
            )
        }
    }

    #[test]
    fn nulled_cursor_can_be_iterated_forwards() {
        let env = LmdbEnv::create_null_with()
            .database("foo", DatabaseStub(42))
            .entry(&[1, 2, 3], &[4, 5, 6])
            .entry(&[2, 2, 2], &[6, 6, 6])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn.txn().open_ro_cursor(DatabaseStub(42)).unwrap();
        let result = cursor.get(None, None, MDB_FIRST);
        assert_eq!(
            result,
            Ok((Some([1u8, 2, 3].as_slice()), [4u8, 5, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [6u8, 6, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(result, Err(lmdb::Error::NotFound));
    }

    #[test]
    fn nulled_cursor_can_be_iterated_backwards() {
        let env = LmdbEnv::create_null_with()
            .database("foo", DatabaseStub(42))
            .entry(&[1, 2, 3], &[4, 5, 6])
            .entry(&[2, 2, 2], &[6, 6, 6])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn.txn().open_ro_cursor(DatabaseStub(42)).unwrap();
        let result = cursor.get(None, None, MDB_LAST);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [6u8, 6, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(
            result,
            Ok((Some([1u8, 2, 3].as_slice()), [4u8, 5, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(result, Err(lmdb::Error::NotFound));
    }

    #[test]
    fn nulled_cursor_can_start_at_specified_key() {
        let env = LmdbEnv::create_null_with()
            .database("foo", DatabaseStub(42))
            .entry(&[1, 1, 1], &[6, 6, 6])
            .entry(&[2, 2, 2], &[7, 7, 7])
            .entry(&[3, 3, 3], &[8, 8, 8])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn.txn().open_ro_cursor(DatabaseStub(42)).unwrap();
        let result = cursor.get(Some([2u8, 2, 2].as_slice()), None, MDB_SET_RANGE);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [7u8, 7, 7].as_slice()))
        );

        let result = cursor.get(Some([2u8, 1, 0].as_slice()), None, MDB_SET_RANGE);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [7u8, 7, 7].as_slice()))
        );
    }

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
