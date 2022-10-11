use std::sync::{Arc, Mutex};

use crate::{
    datastore::{
        frontier_store::FrontierIterator,
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        parallel_traversal, FrontierStore,
    },
    utils::Serialize,
    Account, BlockHash,
};

use super::{
    assert_success, ensure_success, mdb_dbi_open, mdb_del, mdb_get, mdb_put, LmdbEnv,
    LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, Transaction,
};

pub struct LmdbFrontierStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbFrontierStore {
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
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("frontiers"), flags, &mut handle) };

        let mut guard = self.db_handle.lock().unwrap();
        *guard = handle;

        ensure_success(status)
    }
}

impl<'a> FrontierStore<'a, LmdbReadTransaction, LmdbWriteTransaction, LmdbIteratorImpl>
    for LmdbFrontierStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash, account: &Account) {
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from(hash),
                &mut MdbVal::from(account),
                0,
            )
        };
        assert_success(status);
    }

    fn get(&self, txn: &Transaction, hash: &BlockHash) -> Account {
        let mut value = MdbVal::new();
        let status = unsafe {
            mdb_get(
                txn.handle(),
                self.db_handle(),
                &mut MdbVal::from(hash),
                &mut value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        if status == MDB_SUCCESS {
            Account::from_slice(value.as_slice()).unwrap_or_default()
        } else {
            Account::new()
        }
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        let status =
            unsafe { mdb_del(txn.handle, self.db_handle(), &mut MdbVal::from(hash), None) };
        assert_success(status);
    }

    fn begin(&self, txn: &Transaction) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::new(
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
    ) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            MdbVal::from(hash),
            BlockHash::serialized_size(),
            true,
        ))
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction,
            FrontierIterator<LmdbIteratorImpl>,
            FrontierIterator<LmdbIteratorImpl>,
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

    fn end(&self) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::null())
    }
}
