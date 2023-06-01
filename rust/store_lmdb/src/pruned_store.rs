use std::sync::Arc;

use crate::{
    iterator::DbIterator, lmdb_env::EnvironmentWrapper, parallel_traversal, Environment, LmdbEnv,
    LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, RwTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rand::{thread_rng, Rng};
use rsnano_core::{BlockHash, NoValue};

pub type PrunedIterator = Box<dyn DbIterator<BlockHash, NoValue>>;

pub struct LmdbPrunedStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbPrunedStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pruned"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        txn.put(self.database, hash.as_bytes(), &[0; 0], WriteFlags::empty())
            .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        txn.delete(self.database, hash.as_bytes(), None).unwrap();
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> bool {
        txn.exists(self.database, hash.as_bytes())
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> PrunedIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> PrunedIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(hash.as_bytes()), true)
    }

    pub fn random(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Option<BlockHash> {
        let random_hash = BlockHash::from_bytes(thread_rng().gen());
        let mut existing = self.begin_at_hash(txn, &random_hash);
        if existing.is_end() {
            existing = self.begin(txn);
        }

        existing.current().map(|(k, _)| *k)
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

    pub fn end(&self) -> PrunedIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, PrunedIterator, PrunedIterator) + Send + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_hash(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;
    use rsnano_core::NoValue;

    use super::*;

    fn create_sut() -> anyhow::Result<(TestLmdbEnv, LmdbPrunedStore)> {
        let env = TestLmdbEnv::new();
        let store = LmdbPrunedStore::new(env.env())?;
        Ok((env, store))
    }

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let (env, store) = create_sut()?;
        let txn = env.tx_begin_read();

        assert_eq!(store.count(&txn), 0);
        assert_eq!(store.exists(&txn, &BlockHash::from(1)), false);
        assert_eq!(store.begin(&txn).is_end(), true);
        assert!(store.random(&txn).is_none());
        Ok(())
    }

    #[test]
    fn add_one() -> anyhow::Result<()> {
        let (env, store) = create_sut()?;
        let mut txn = env.tx_begin_write();

        let hash = BlockHash::from(1);
        store.put(&mut txn, &hash);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.exists(&txn, &hash), true);
        assert_eq!(store.begin(&txn).current(), Some((&hash, &NoValue {})));
        assert_eq!(store.random(&txn), Some(hash));
        Ok(())
    }

    #[test]
    fn add_two() -> anyhow::Result<()> {
        let (env, store) = create_sut()?;
        let mut txn = env.tx_begin_write();

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        store.put(&mut txn, &hash1);
        store.put(&mut txn, &hash2);

        assert_eq!(store.count(&txn), 2);
        assert_eq!(store.exists(&txn, &hash1), true);
        assert_eq!(store.exists(&txn, &hash2), true);
        Ok(())
    }

    #[test]
    fn add_delete() -> anyhow::Result<()> {
        let (env, store) = create_sut()?;
        let mut txn = env.tx_begin_write();

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        store.put(&mut txn, &hash1);
        store.put(&mut txn, &hash2);
        store.del(&mut txn, &hash1);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.exists(&txn, &hash1), false);
        assert_eq!(store.exists(&txn, &hash2), true);
        Ok(())
    }

    #[test]
    fn pruned_random() -> anyhow::Result<()> {
        let (env, store) = create_sut()?;
        let mut txn = env.tx_begin_write();
        let hash = BlockHash::random();
        store.put(&mut txn, &hash);
        let random_hash = store.random(&txn);
        assert_eq!(random_hash, Some(hash));
        Ok(())
    }
}
