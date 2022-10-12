use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, Transaction, WriteFlags};

use crate::datastore::{VersionStore, STORE_VERSION_MINIMUM};

use super::{LmdbEnv, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction};

pub struct LmdbVersionStore {
    env: Arc<LmdbEnv>,

    /// U256 (arbitrary key) -> blob
    db_handle: Mutex<Option<Database>>,
}

impl LmdbVersionStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(None),
        }
    }

    pub fn open_db(&self, txn: &LmdbReadTransaction) -> anyhow::Result<()> {
        let mut guard = self.db_handle.lock().unwrap();
        *guard = Some(unsafe { txn.txn().open_db(Some("meta")) }?);
        Ok(())
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        let mut guard = self.db_handle.lock().unwrap();
        *guard = Some(
            self.env
                .environment
                .create_db(Some("meta"), DatabaseFlags::empty())?,
        );
        Ok(())
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle.lock().unwrap().unwrap()
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
        let key_bytes = version_key();
        match txn.get(db, &key_bytes) {
            Ok(value) => i32::from_ne_bytes(value[28..].try_into().unwrap()),
            Err(_) => STORE_VERSION_MINIMUM,
        }
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
