use std::sync::Arc;

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    core::Amount,
    ledger::datastore::{online_weight_store::OnlineWeightIterator, OnlineWeightStore},
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::datastore::lmdb::TestLmdbEnv;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert_eq!(store.count(&txn.as_txn()), 0);
        assert!(store.begin(&txn.as_txn()).is_end());
        assert!(store.rbegin(&txn.as_txn()).is_end());
        Ok(())
    }

    #[test]
    fn add_one() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let time = 1;
        let amount = Amount::new(2);
        store.put(&mut txn, time, &amount);

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(store.begin(&txn.as_txn()).current(), Some((&time, &amount)));
        assert_eq!(
            store.rbegin(&txn.as_txn()).current(),
            Some((&time, &amount))
        );
        Ok(())
    }

    #[test]
    fn add_two() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let time1 = 1;
        let time2 = 2;
        let amount1 = Amount::new(3);
        let amount2 = Amount::new(4);
        store.put(&mut txn, time1, &amount1);
        store.put(&mut txn, time2, &amount2);

        assert_eq!(store.count(&txn.as_txn()), 2);
        assert_eq!(
            store.begin(&txn.as_txn()).current(),
            Some((&time1, &amount1))
        );
        assert_eq!(
            store.rbegin(&txn.as_txn()).current(),
            Some((&time2, &amount2))
        );
        Ok(())
    }

    #[test]
    fn delete() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let time1 = 1;
        let time2 = 2;
        let amount1 = Amount::new(3);
        let amount2 = Amount::new(4);
        store.put(&mut txn, time1, &amount1);
        store.put(&mut txn, time2, &amount2);

        store.del(&mut txn, time1);

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(
            store.begin(&txn.as_txn()).current(),
            Some((&time2, &amount2))
        );
        Ok(())
    }
}
