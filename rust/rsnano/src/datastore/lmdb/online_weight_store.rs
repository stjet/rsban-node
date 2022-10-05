use std::sync::{Arc, Mutex};

use crate::{
    datastore::{DbIterator, OnlineWeightStore},
    Amount,
};

use super::{
    assert_success, ensure_success, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, Transaction,
};

pub struct LmdbOnlineWeightStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbOnlineWeightStore {
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
        let status =
            unsafe { mdb_dbi_open(txn.handle(), Some("online_weight"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl OnlineWeightStore<LmdbReadTransaction, LmdbWriteTransaction> for LmdbOnlineWeightStore {
    fn put(&self, txn: &LmdbWriteTransaction, time: u64, amount: &Amount) {
        let time_bytes = time.to_be_bytes();
        let amount_bytes = amount.to_be_bytes();
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&time_bytes),
                &mut MdbVal::from_slice(&amount_bytes),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &LmdbWriteTransaction, time: u64) {
        let time_bytes = time.to_be_bytes();
        let status = unsafe {
            mdb_del(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&time_bytes),
                None,
            )
        };
        assert_success(status);
    }

    fn begin(&self, txn: &Transaction) -> Box<dyn DbIterator<u64, Amount>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), None, true))
    }

    fn rbegin(&self, txn: &Transaction) -> Box<dyn DbIterator<u64, Amount>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), None, false))
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }

    fn clear(&self, txn: &LmdbWriteTransaction) {
        let status = unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
        assert_success(status);
    }
}
