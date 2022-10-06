use std::sync::{Arc, Mutex};

use crate::{datastore::PeerStore, EndpointKey, NoValue};

use super::{
    assert_success, ensure_success, exists, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_put,
    LmdbEnv, LmdbIterator, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, OwnedMdbVal,
    Transaction,
};

pub struct LmdbPeerStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbPeerStore {
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
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("peers"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl PeerStore<LmdbReadTransaction, LmdbWriteTransaction, LmdbIterator<EndpointKey, NoValue>>
    for LmdbPeerStore
{
    fn put(&self, txn: &LmdbWriteTransaction, endpoint: &EndpointKey) {
        let mut key = OwnedMdbVal::from(endpoint);
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                key.as_mdb_val(),
                &mut MdbVal::new(),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &LmdbWriteTransaction, endpoint: &EndpointKey) {
        let mut key = OwnedMdbVal::from(endpoint);
        let status = unsafe { mdb_del(txn.handle, self.db_handle(), key.as_mdb_val(), None) };
        assert_success(status);
    }

    fn exists(&self, txn: &Transaction, endpoint: &EndpointKey) -> bool {
        let mut key = OwnedMdbVal::from(endpoint);
        exists(txn, self.db_handle(), key.as_mdb_val())
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }

    fn clear(&self, txn: &LmdbWriteTransaction) {
        let status = unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
        assert_success(status);
    }

    fn begin(&self, txn: &Transaction) -> LmdbIterator<EndpointKey, NoValue> {
        LmdbIterator::new(txn, self.db_handle(), None, true)
    }
}
