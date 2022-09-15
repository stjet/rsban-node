use std::sync::Arc;

use crate::{
    datastore::{DbIterator, OnlineWeightStore, Transaction, WriteTransaction},
    Amount,
};

use super::{
    assert_success, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv, LmdbIterator,
    MdbVal,
};

pub struct LmdbOnlineWeightStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbOnlineWeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl OnlineWeightStore for LmdbOnlineWeightStore {
    fn put(&self, txn: &dyn WriteTransaction, time: u64, amount: &Amount) {
        let time_bytes = time.to_be_bytes();
        let amount_bytes = amount.to_be_bytes();
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from_slice(&time_bytes),
                &mut MdbVal::from_slice(&amount_bytes),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &dyn WriteTransaction, time: u64) {
        let time_bytes = time.to_be_bytes();
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from_slice(&time_bytes),
                None,
            )
        };
        assert_success(status);
    }

    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<u64, Amount>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, true))
    }

    fn rbegin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<u64, Amount>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, false))
    }

    fn count(&self, txn: &dyn Transaction) -> usize {
        unsafe { mdb_count(get_raw_lmdb_txn(txn), self.table_handle) }
    }

    fn clear(&self, txn: &dyn WriteTransaction) {
        let status =
            unsafe { mdb_drop(get_raw_lmdb_txn(txn.as_transaction()), self.table_handle, 0) };
        assert_success(status);
    }
}
