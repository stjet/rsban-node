use super::{LmdbEnv, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction};
use crate::datastore::{VersionStore, STORE_VERSION_MINIMUM};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use std::sync::Arc;

pub struct LmdbVersionStore {
    env: Arc<LmdbEnv>,

    /// U256 (arbitrary key) -> blob
    db_handle: Database,
}

impl LmdbVersionStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let db_handle = env
            .environment
            .create_db(Some("meta"), DatabaseFlags::empty())?;
        Ok(Self { env, db_handle })
    }

    pub fn try_read_version(env: &LmdbEnv) -> Option<i32> {
        match env.environment.open_db(Some("meta")) {
            Ok(db) => {
                let txn = env.tx_begin_read().unwrap();
                Some(load_version(&txn.as_txn(), db))
            }
            Err(_) => None,
        }
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle
    }
}

impl<'a> VersionStore<LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>> for LmdbVersionStore {
    fn put(&self, txn: &mut LmdbWriteTransaction, version: i32) {
        let db = self.db_handle();

        let key_bytes = version_key();
        let value_bytes = value_bytes(version);

        txn.rw_txn_mut()
            .put(db, &key_bytes, &value_bytes, WriteFlags::empty())
            .unwrap();
    }

    fn get(&self, txn: &LmdbTransaction) -> i32 {
        let db = self.db_handle();
        load_version(txn, db)
    }
}

fn load_version(txn: &LmdbTransaction, db: Database) -> i32 {
    let key_bytes = version_key();
    match txn.get(db, &key_bytes) {
        Ok(value) => i32::from_ne_bytes(value[28..].try_into().unwrap()),
        Err(_) => STORE_VERSION_MINIMUM,
    }
}

fn value_bytes(version: i32) -> [u8; 32] {
    let mut value_bytes = [0; 32];
    value_bytes[28..].copy_from_slice(&version.to_ne_bytes());
    value_bytes
}

fn version_key() -> [u8; 32] {
    value_bytes(1)
}
