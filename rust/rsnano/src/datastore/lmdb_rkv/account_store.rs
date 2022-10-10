use crate::{
    datastore::{parallel_traversal, AccountIterator, AccountStore, DbIterator2},
    utils::{Deserialize, StreamAdapter},
    Account, AccountInfo,
};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use std::sync::{Arc, Mutex};

use super::{
    iterator::LmdbIteratorImpl, LmdbEnv, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbAccountStore {
    env: Arc<LmdbEnv>,

    /// U256 (arbitrary key) -> blob
    db_handle: Mutex<Option<Database>>,
}

impl LmdbAccountStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(None),
        }
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle.lock().unwrap().unwrap()
    }

    pub fn create_db(&self) -> lmdb::Result<()> {
        *self.db_handle.lock().unwrap() = Some(
            self.env
                .environment
                .create_db(Some("accounts"), DatabaseFlags::empty())?,
        );
        Ok(())
    }
}

impl<'a> AccountStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbAccountStore
{
    fn put(
        &self,
        transaction: &mut LmdbWriteTransaction,
        account: &crate::Account,
        info: &crate::AccountInfo,
    ) {
        transaction
            .rw_txn()
            .put(
                self.db_handle(),
                account.as_bytes(),
                &info.to_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn get(&self, transaction: &LmdbTransaction, account: &Account) -> Option<AccountInfo> {
        let result = transaction.get(self.db_handle(), account.as_bytes());
        match result {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                AccountInfo::deserialize(&mut stream).ok()
            }
            Err(e) => panic!("Could not load account info {:?}", e),
        }
    }

    fn del(&self, transaction: &mut LmdbWriteTransaction, account: &Account) {
        transaction
            .rw_txn()
            .del(self.db_handle(), account.as_bytes(), None)
            .unwrap();
    }

    fn begin_account(
        &self,
        transaction: &LmdbTransaction,
        account: &Account,
    ) -> DbIterator2<Account, AccountInfo, LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::new(
            transaction,
            self.db_handle(),
            Some(account.as_bytes()),
            true,
        ))
    }

    fn begin(&self, transaction: &LmdbTransaction) -> AccountIterator<LmdbIteratorImpl> {
        AccountIterator::new(LmdbIteratorImpl::new(
            transaction,
            self.db_handle(),
            None,
            true,
        ))
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            AccountIterator<LmdbIteratorImpl>,
            AccountIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let txn = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_account(&txn.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_account(&txn.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(txn, begin_it, end_it);
        })
    }

    fn end(&self) -> AccountIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::null())
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.db_handle())
    }
}
