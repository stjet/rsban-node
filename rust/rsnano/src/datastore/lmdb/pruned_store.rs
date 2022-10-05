use std::sync::{Arc, Mutex};

use rand::{thread_rng, Rng};

use crate::{
    datastore::{parallel_traversal, DbIterator, NullIterator, PrunedStore},
    BlockHash, NoValue,
};

use super::{
    assert_success, ensure_success, exists, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_put,
    LmdbEnv, LmdbIterator, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, Transaction,
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

impl PrunedStore<LmdbReadTransaction, LmdbWriteTransaction> for LmdbPrunedStore {
    fn put(&self, txn: &LmdbWriteTransaction, hash: &BlockHash) {
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

    fn del(&self, txn: &LmdbWriteTransaction, hash: &BlockHash) {
        let status =
            unsafe { mdb_del(txn.handle, self.db_handle(), &mut MdbVal::from(hash), None) };
        assert_success(status);
    }

    fn exists(&self, txn: &Transaction, hash: &BlockHash) -> bool {
        exists(txn, self.db_handle(), &mut hash.into())
    }

    fn begin(&self, txn: &Transaction) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_hash(
        &self,
        txn: &Transaction,
        hash: &BlockHash,
    ) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), Some(hash), true))
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

    fn clear(&self, txn: &LmdbWriteTransaction) {
        let status = unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
        assert_success(status);
    }

    fn end(&self) -> Box<dyn DbIterator<BlockHash, NoValue>> {
        Box::new(NullIterator::new())
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &LmdbReadTransaction,
            &mut dyn DbIterator<BlockHash, NoValue>,
            &mut dyn DbIterator<BlockHash, NoValue>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let mut transaction = self.env.tx_begin_read();
            let mut begin_it = self.begin_at_hash(&transaction.as_txn(), &start.into());
            let mut end_it = if !is_last {
                self.begin_at_hash(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(&mut transaction, begin_it.as_mut(), end_it.as_mut());
        });
    }
}
