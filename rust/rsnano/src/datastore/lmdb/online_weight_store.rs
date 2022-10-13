use std::sync::Arc;

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
    database: Database,
}

impl LmdbOnlineWeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("online_weight"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
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
                self.database,
                &time_bytes,
                &amount_bytes,
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, time: u64) {
        let time_bytes = time.to_be_bytes();
        txn.rw_txn_mut()
            .del(self.database, &time_bytes, None)
            .unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> OnlineWeightIterator<LmdbIteratorImpl> {
        OnlineWeightIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn rbegin(&self, txn: &LmdbTransaction) -> OnlineWeightIterator<LmdbIteratorImpl> {
        OnlineWeightIterator::new(LmdbIteratorImpl::new(txn, self.database, None, false))
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }
}
