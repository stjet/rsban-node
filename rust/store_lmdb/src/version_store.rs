use crate::{as_write_txn, get, LmdbEnv, STORE_VERSION_CURRENT, EnvironmentStrategy, EnvironmentWrapper};
use core::panic;
use lmdb::{Database, DatabaseFlags, WriteFlags};
use rsnano_store_traits::{Transaction, VersionStore, WriteTransaction};
use std::{path::Path, sync::Arc};

pub struct LmdbVersionStore<T: EnvironmentStrategy = EnvironmentWrapper> {
    _env: Arc<LmdbEnv<T>>,

    /// U256 (arbitrary key) -> blob
    db_handle: Database,
}

pub struct UpgradeInfo {
    pub is_fresh_db: bool,
    pub is_fully_upgraded: bool,
}

impl<T: EnvironmentStrategy + 'static> LmdbVersionStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let db_handle = env
            .environment
            .create_db(Some("meta"), DatabaseFlags::empty())?;
        Ok(Self {
            _env: env,
            db_handle,
        })
    }

    pub fn try_read_version(env: &LmdbEnv<T>) -> anyhow::Result<Option<i32>> {
        match env.environment.open_db(Some("meta")) {
            Ok(db) => {
                let txn = env.tx_begin_read()?;
                Ok(load_version::<T>(&txn, db))
            }
            Err(_) => Ok(None),
        }
    }

    pub fn check_upgrade(path: &Path) -> anyhow::Result<UpgradeInfo> {
        let env = LmdbEnv::<T>::new(path)?;
        let info = match LmdbVersionStore::try_read_version(&env)? {
            Some(version) => UpgradeInfo {
                is_fresh_db: false,
                is_fully_upgraded: version == STORE_VERSION_CURRENT,
            },
            None => UpgradeInfo {
                is_fresh_db: true,
                is_fully_upgraded: false,
            },
        };
        Ok(info)
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle
    }
}

impl<T: EnvironmentStrategy + 'static> VersionStore for LmdbVersionStore<T> {
    fn put(&self, txn: &mut dyn WriteTransaction, version: i32) {
        let db = self.db_handle();

        let key_bytes = version_key();
        let value_bytes = value_bytes(version);

        as_write_txn::<T>(txn)
            .put(db, &key_bytes, &value_bytes, WriteFlags::empty())
            .unwrap();
    }

    fn get(&self, txn: &dyn Transaction) -> Option<i32> {
        let db = self.db_handle();
        load_version::<T>(txn, db)
    }
}

fn load_version<T: EnvironmentStrategy + 'static>(txn: &dyn Transaction, db: Database) -> Option<i32> {
    let key_bytes = version_key();
    match get::<T, _>(txn, db, &key_bytes) {
        Ok(value) => Some(i32::from_be_bytes(value[28..].try_into().unwrap())),
        Err(lmdb::Error::NotFound) => None,
        Err(_) => panic!("Error while loading db version"),
    }
}

fn value_bytes(version: i32) -> [u8; 32] {
    let mut value_bytes = [0; 32];
    value_bytes[28..].copy_from_slice(&version.to_be_bytes());
    value_bytes
}

fn version_key() -> [u8; 32] {
    value_bytes(1)
}
