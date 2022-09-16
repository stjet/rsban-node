use std::sync::Arc;

use crate::{
    datastore::{DbIterator, PeerStore, Transaction, WriteTransaction},
    EndpointKey, NoValue,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal, OwnedMdbVal,
};

pub struct LmdbPeerStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbPeerStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl PeerStore for LmdbPeerStore {
    fn put(&self, txn: &dyn WriteTransaction, endpoint: &EndpointKey) {
        let mut key = OwnedMdbVal::from(endpoint);
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                key.as_mdb_val(),
                &mut MdbVal::new(),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &dyn WriteTransaction, endpoint: &EndpointKey) {
        let mut key = OwnedMdbVal::from(endpoint);
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                key.as_mdb_val(),
                None,
            )
        };
        assert_success(status);
    }

    fn exists(&self, txn: &dyn Transaction, endpoint: &EndpointKey) -> bool {
        let mut key = OwnedMdbVal::from(endpoint);
        exists(txn, self.table_handle, key.as_mdb_val())
    }

    fn count(&self, txn: &dyn Transaction) -> usize {
        unsafe { mdb_count(get_raw_lmdb_txn(txn), self.table_handle) }
    }

    fn clear(&self, txn: &dyn WriteTransaction) {
        let status =
            unsafe { mdb_drop(get_raw_lmdb_txn(txn.as_transaction()), self.table_handle, 0) };
        assert_success(status);
    }

    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<EndpointKey, NoValue>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, true))
    }
}
