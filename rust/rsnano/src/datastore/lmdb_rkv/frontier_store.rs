use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{frontier_store::FrontierIterator, parallel_traversal, FrontierStore},
    Account, BlockHash,
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbFrontierStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbFrontierStore {
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
            .create_db(Some("frontiers"), DatabaseFlags::empty())
            .unwrap();
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a> FrontierStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbFrontierStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash, account: &Account) {
        txn.rw_txn_mut()
            .put(
                self.db_handle(),
                hash.as_bytes(),
                account.as_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn get(&self, txn: &LmdbTransaction, hash: &BlockHash) -> Account {
        match txn.get(self.db_handle(), hash.as_bytes()) {
            Ok(bytes) => Account::from_slice(bytes).unwrap_or_default(),
            Err(lmdb::Error::NotFound) => Account::new(),
            Err(e) => panic!("Could not load frontier: {:?}", e),
        }
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .del(self.db_handle(), hash.as_bytes(), None)
            .unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_hash(
        &self,
        txn: &LmdbTransaction,
        hash: &BlockHash,
    ) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            Some(hash.as_bytes()),
            true,
        ))
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            FrontierIterator<LmdbIteratorImpl>,
            FrontierIterator<LmdbIteratorImpl>,
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

    fn end(&self) -> FrontierIterator<LmdbIteratorImpl> {
        FrontierIterator::new(LmdbIteratorImpl::null())
    }
}
