use std::sync::{Arc, Mutex};

use rand::{thread_rng, Rng};

use crate::{
    datastore::{parallel_traversal, pruned_store::PrunedIterator, PrunedStore},
    utils::Serialize,
    BlockHash,
};

use super::{
    assert_success, ensure_success, exists, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_put,
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, Transaction,
};

pub struct LmdbPrunedStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbPrunedStore {
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
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("pruned"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl<'a> PrunedStore<'a, LmdbReadTransaction, LmdbWriteTransaction, LmdbIteratorImpl>
    for LmdbPrunedStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from(hash),
                &mut MdbVal::new(),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        let status =
            unsafe { mdb_del(txn.handle, self.db_handle(), &mut MdbVal::from(hash), None) };
        assert_success(status);
    }

    fn exists(&self, txn: &Transaction, hash: &BlockHash) -> bool {
        exists(txn, self.db_handle(), &mut hash.into())
    }

    fn begin(&self, txn: &Transaction) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            MdbVal::new(),
            BlockHash::serialized_size(),
            true,
        ))
    }

    fn begin_at_hash(
        &self,
        txn: &Transaction,
        hash: &BlockHash,
    ) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            MdbVal::from(hash),
            BlockHash::serialized_size(),
            true,
        ))
    }

    fn random(&self, txn: &Transaction) -> BlockHash {
        let random_hash = BlockHash::from_bytes(thread_rng().gen());
        let mut existing = self.begin_at_hash(txn, &random_hash);
        if existing.is_end() {
            existing = self.begin(txn);
        }

        let result = existing.current().map(|(k, _)| *k).unwrap_or_default();
        result
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        let status = unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
        assert_success(status);
    }

    fn end(&self) -> PrunedIterator<LmdbIteratorImpl> {
        PrunedIterator::new(LmdbIteratorImpl::null())
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            LmdbReadTransaction,
            PrunedIterator<LmdbIteratorImpl>,
            PrunedIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
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
