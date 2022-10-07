use crate::{
    datastore::{AccountIterator, AccountStore, DbIterator2},
    Account, AccountInfo,
};
use lmdb::Database;
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
}

impl<'a> AccountStore<LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl<'a>>
    for LmdbAccountStore
{
    fn put(
        &self,
        transaction: &LmdbWriteTransaction,
        account: &crate::Account,
        info: &crate::AccountInfo,
    ) {
        todo!()
    }

    fn get(
        &self,
        transaction: &crate::datastore::Transaction<LmdbReadTransaction, LmdbWriteTransaction>,
        account: &crate::Account,
    ) -> Option<crate::AccountInfo> {
        todo!()
    }

    fn del(&self, transaction: &LmdbWriteTransaction, account: &crate::Account) {
        todo!()
    }

    fn begin_account(
        &self,
        transaction: &LmdbTransaction,
        account: &Account,
    ) -> DbIterator2<Account, AccountInfo, LmdbIteratorImpl<'a>> {
        todo!()
    }

    fn begin(&self, transaction: &LmdbTransaction) -> AccountIterator<LmdbIteratorImpl<'a>> {
        todo!()
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &LmdbReadTransaction<'a>,
            AccountIterator<LmdbIteratorImpl<'a>>,
            AccountIterator<LmdbIteratorImpl<'a>>,
        ) + Send
              + Sync),
    ) {
        todo!()
    }

    fn end(&self) -> AccountIterator<LmdbIteratorImpl<'a>> {
        todo!()
    }

    fn count(
        &self,
        txn: &crate::datastore::Transaction<LmdbReadTransaction, LmdbWriteTransaction>,
    ) -> usize {
        todo!()
    }
}
