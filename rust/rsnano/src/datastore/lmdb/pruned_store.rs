use std::sync::Arc;

use lmdb::{Database, DatabaseFlags, WriteFlags};
use rand::{thread_rng, Rng};

use crate::{
    core::BlockHash,
    datastore::{parallel_traversal, pruned_store::PrunedIterator, PrunedStore},
};

use super::{
    exists, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbPrunedStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbPrunedStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pruned"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }
}

impl<'a> PrunedStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbPrunedStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .put(self.database, hash.as_bytes(), &[0; 0], WriteFlags::empty())
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .del(self.database, hash.as_bytes(), None)
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, hash: &BlockHash) -> bool {
        exists(txn, self.database, hash.as_bytes())
    }

    fn begin(&self, txn: &LmdbTransaction) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn begin_at_hash(
        &self,
        txn: &LmdbTransaction,
        hash: &BlockHash,
    ) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.database,
            Some(hash.as_bytes()),
            true,
        ))
    }

    fn random(&self, txn: &LmdbTransaction) -> BlockHash {
        let random_hash = BlockHash::from_bytes(thread_rng().gen());
        let mut existing = self.begin_at_hash(txn, &random_hash);
        if existing.is_end() {
            existing = self.begin(txn);
        }

        let result = existing.current().map(|(k, _)| *k).unwrap_or_default();
        result
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }

    fn end(&self) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::null())
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            PrunedIterator<LmdbIteratorImpl>,
            PrunedIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_hash(&transaction.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it, end_it);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{core::NoValue, datastore::lmdb::TestLmdbEnv};

    use super::*;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPrunedStore::new(env.env())?;
        let txn = env.tx_begin_read()?;

        assert_eq!(store.count(&txn.as_txn()), 0);
        assert_eq!(store.exists(&txn.as_txn(), &BlockHash::from(1)), false);
        assert_eq!(store.begin(&txn.as_txn()).is_end(), true);
        assert_eq!(store.random(&txn.as_txn()), BlockHash::zero());
        Ok(())
    }

    #[test]
    fn add_one() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPrunedStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let hash = BlockHash::from(1);
        store.put(&mut txn, &hash);

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(store.exists(&txn.as_txn(), &hash), true);
        assert_eq!(
            store.begin(&txn.as_txn()).current(),
            Some((&hash, &NoValue {}))
        );
        assert_eq!(store.random(&txn.as_txn()), hash);
        Ok(())
    }

    #[test]
    fn add_two() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPrunedStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        store.put(&mut txn, &hash1);
        store.put(&mut txn, &hash2);

        assert_eq!(store.count(&txn.as_txn()), 2);
        assert_eq!(store.exists(&txn.as_txn(), &hash1), true);
        assert_eq!(store.exists(&txn.as_txn(), &hash2), true);
        Ok(())
    }

    #[test]
    fn add_delete() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPrunedStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        store.put(&mut txn, &hash1);
        store.put(&mut txn, &hash2);
        store.del(&mut txn, &hash1);

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(store.exists(&txn.as_txn(), &hash1), false);
        assert_eq!(store.exists(&txn.as_txn(), &hash2), true);
        Ok(())
    }
}
