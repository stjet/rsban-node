use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};
use rand::{thread_rng, Rng};

use crate::{
    datastore::{parallel_traversal, pruned_store::PrunedIterator, PrunedStore},
    BlockHash,
};

use super::{
    exists, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbPrunedStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbPrunedStore {
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
            .create_db(Some("pruned"), DatabaseFlags::empty())
            .unwrap();
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a> PrunedStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbPrunedStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .put(
                self.db_handle(),
                hash.as_bytes(),
                &[0; 0],
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .del(self.db_handle(), hash.as_bytes(), None)
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, hash: &BlockHash) -> bool {
        exists(txn, self.db_handle(), hash.as_bytes())
    }

    fn begin(&self, txn: &LmdbTransaction) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_hash(
        &self,
        txn: &LmdbTransaction,
        hash: &BlockHash,
    ) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
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
        txn.count(self.db_handle())
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.db_handle()).unwrap();
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
