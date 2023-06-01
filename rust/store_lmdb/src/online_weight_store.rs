use std::sync::Arc;

use crate::{
    iterator::DbIterator, lmdb_env::RwTransaction, Environment, EnvironmentWrapper, LmdbEnv,
    LmdbIteratorImpl, LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::Amount;

pub type OnlineWeightIterator = Box<dyn DbIterator<u64, Amount>>;

pub struct LmdbOnlineWeightStore<T: Environment = EnvironmentWrapper> {
    _env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbOnlineWeightStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("online_weight"), DatabaseFlags::empty())?;
        Ok(Self {
            _env: env,
            database,
        })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, time: u64, amount: &Amount) {
        let time_bytes = time.to_be_bytes();
        let amount_bytes = amount.to_be_bytes();
        txn.put(
            self.database,
            &time_bytes,
            &amount_bytes,
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, time: u64) {
        let time_bytes = time.to_be_bytes();
        txn.delete(self.database, &time_bytes, None).unwrap();
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> OnlineWeightIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn rbegin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> OnlineWeightIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, false)
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.clear_db(self.database).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;

    use super::*;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let txn = env.tx_begin_read();
        assert_eq!(store.count(&txn), 0);
        assert!(store.begin(&txn).is_end());
        assert!(store.rbegin(&txn).is_end());
        Ok(())
    }

    #[test]
    fn add_one() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write();

        let time = 1;
        let amount = Amount::raw(2);
        store.put(&mut txn, time, &amount);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.begin(&txn).current(), Some((&time, &amount)));
        assert_eq!(store.rbegin(&txn).current(), Some((&time, &amount)));
        Ok(())
    }

    #[test]
    fn add_two() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write();

        let time1 = 1;
        let time2 = 2;
        let amount1 = Amount::raw(3);
        let amount2 = Amount::raw(4);
        store.put(&mut txn, time1, &amount1);
        store.put(&mut txn, time2, &amount2);

        assert_eq!(store.count(&txn), 2);
        assert_eq!(store.begin(&txn).current(), Some((&time1, &amount1)));
        assert_eq!(store.rbegin(&txn).current(), Some((&time2, &amount2)));
        Ok(())
    }

    #[test]
    fn delete() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbOnlineWeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write();

        let time1 = 1;
        let time2 = 2;
        let amount1 = Amount::raw(3);
        let amount2 = Amount::raw(4);
        store.put(&mut txn, time1, &amount1);
        store.put(&mut txn, time2, &amount2);

        store.del(&mut txn, time1);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.begin(&txn).current(), Some((&time2, &amount2)));
        Ok(())
    }
}
