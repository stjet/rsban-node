use std::{
    convert::TryInto,
    sync::{Arc, Mutex},
};

use crate::datastore::{VersionStore, STORE_VERSION_MINIMUM};

use super::{
    assert_success, ensure_success, mdb_dbi_open, mdb_get, mdb_put, LmdbEnv, LmdbReadTransaction,
    LmdbWriteTransaction, MdbVal, Transaction, MDB_SUCCESS,
};

pub struct LmdbVersionStore {
    env: Arc<LmdbEnv>,

    /// U256 (arbitrary key) -> blob
    db_handle: Mutex<u32>,
}

impl LmdbVersionStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(0),
        }
    }

    pub fn db_handle(&self) -> u32 {
        *self.db_handle.lock().unwrap()
    }

    pub fn open_db(&self, txn: &Transaction, flags: u32) -> anyhow::Result<()> {
        let mut handle = 0;
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("meta"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl VersionStore<LmdbReadTransaction, LmdbWriteTransaction> for LmdbVersionStore {
    fn put(&self, txn: &mut LmdbWriteTransaction, version: i32) {
        let key_bytes = version_key();
        let value_bytes = value_bytes(version);

        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                &mut MdbVal::from_slice(&value_bytes),
                0,
            )
        };
        assert_success(status);
    }

    fn get(&self, txn: &Transaction) -> i32 {
        let key_bytes = version_key();
        let mut data = MdbVal::new();
        let status = unsafe {
            mdb_get(
                txn.handle(),
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                &mut data,
            )
        };
        if status == MDB_SUCCESS {
            i32::from_ne_bytes(data.as_slice()[28..].try_into().unwrap())
        } else {
            STORE_VERSION_MINIMUM
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
