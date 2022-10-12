use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{online_weight_store::OnlineWeightIterator, OnlineWeightStore},
    Amount,
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbOnlineWeightStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbOnlineWeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(None),
        }
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle.lock().unwrap().unwrap()
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        let db = self
            .env
            .environment
            .create_db(Some("online_weight"), DatabaseFlags::empty())?;
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a> OnlineWeightStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbOnlineWeightStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, time: u64, amount: &Amount) {
        let time_bytes = time.to_be_bytes();
        let amount_bytes = amount.to_be_bytes();
        txn.rw_txn_mut()
            .put(
                self.db_handle(),
                &time_bytes,
                &amount_bytes,
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, time: u64) {
        let time_bytes = time.to_be_bytes();
        txn.rw_txn_mut()
            .del(self.db_handle(), &time_bytes, None)
            .unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> OnlineWeightIterator<LmdbIteratorImpl> {
        OnlineWeightIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }

    fn rbegin(&self, txn: &LmdbTransaction) -> OnlineWeightIterator<LmdbIteratorImpl> {
        OnlineWeightIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, false))
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.db_handle())
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.db_handle()).unwrap();
    }
}
