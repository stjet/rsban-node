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
            .rw_txn_mut()
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
            .rw_txn_mut()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{datastore::lmdb_rkv::TestLmdbEnv, Amount, BlockHash};

    struct AccountStoreTestContext {
        pub store: LmdbAccountStore,
        pub env: TestLmdbEnv,
    }

    impl AccountStoreTestContext {
        pub fn new() -> Self {
            let env = TestLmdbEnv::new();
            let store = LmdbAccountStore::new(env.env());
            store.create_db().unwrap();
            Self { store, env }
        }
    }

    #[test]
    fn create_db() {
        let env = TestLmdbEnv::new();
        let store = LmdbAccountStore::new(env.env());
        assert_eq!(store.create_db(), Ok(()));
    }

    #[test]
    fn account_not_found() {
        let sut = AccountStoreTestContext::new();
        let txn = sut.env.tx_begin_read().unwrap();
        let result = sut.store.get(&txn.as_txn(), &Account::from(1));
        assert_eq!(result, None);
    }

    #[test]
    fn put_and_get_account() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        let account = Account::from(1);
        let info = AccountInfo {
            balance: Amount::new(123),
            ..Default::default()
        };
        sut.store.put(&mut txn, &account, &info);
        let result = sut.store.get(&txn.as_txn(), &account);
        assert_eq!(result, Some(info));
    }

    #[test]
    fn del() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        let account = Account::from(1);
        let info = AccountInfo {
            balance: Amount::new(123),
            ..Default::default()
        };
        sut.store.put(&mut txn, &account, &info);
        sut.store.del(&mut txn, &account);
        let result = sut.store.get(&txn.as_txn(), &account);
        assert_eq!(result, None);
    }

    #[test]
    fn count() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        assert_eq!(sut.store.count(&txn.as_txn()), 0);

        sut.store
            .put(&mut txn, &Account::from(1), &AccountInfo::default());
        assert_eq!(sut.store.count(&txn.as_txn()), 1);
    }

    #[test]
    fn begin_empty_store() {
        let sut = AccountStoreTestContext::new();
        let txn = sut.env.tx_begin_read().unwrap();
        let it = sut.store.begin(&txn.as_txn());
        assert!(it.is_end())
    }

    #[test]
    fn begin() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        let account1 = Account::from(1);
        let account2 = Account::from(2);
        let info1 = AccountInfo {
            head: BlockHash::from(1),
            ..Default::default()
        };
        let info2 = AccountInfo {
            head: BlockHash::from(2),
            ..Default::default()
        };
        sut.store.put(&mut txn, &account1, &info1);
        sut.store.put(&mut txn, &account2, &info2);
        let mut it = sut.store.begin(&txn.as_txn());
        assert_eq!(it.current(), Some((&account1, &info1)));
        it.next();
        assert_eq!(it.current(), Some((&account2, &info2)));
        it.next();
        assert_eq!(it.current(), None);
    }

    #[test]
    fn begin_account() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        let account1 = Account::from(1);
        let account3 = Account::from(3);
        let info1 = AccountInfo {
            head: BlockHash::from(1),
            ..Default::default()
        };
        let info3 = AccountInfo {
            head: BlockHash::from(3),
            ..Default::default()
        };
        sut.store.put(&mut txn, &account1, &info1);
        sut.store.put(&mut txn, &account3, &info3);
        let mut it = sut.store.begin_account(&txn.as_txn(), &Account::from(2));
        assert_eq!(it.current(), Some((&account3, &info3)));
        it.next();
        assert_eq!(it.current(), None);
    }

    #[test]
    fn for_each_par() {
        let sut = AccountStoreTestContext::new();
        let mut txn = sut.env.tx_begin_write().unwrap();
        let account_1 = Account::from(1);
        let account_max = Account::from_bytes([0xFF; 32]);
        let info_1 = AccountInfo {
            balance: Amount::new(1),
            ..Default::default()
        };
        let info_max = AccountInfo {
            balance: Amount::new(3),
            ..Default::default()
        };
        sut.store.put(&mut txn, &account_1, &info_1);
        sut.store.put(&mut txn, &account_max, &info_max);
        txn.commit();

        let balance_sum = Mutex::new(Amount::zero());
        sut.store.for_each_par(&|_, mut begin, end| {
            while begin != end {
                if let Some((_, v)) = begin.current() {
                    *balance_sum.lock().unwrap() += v.balance
                }
                begin.next();
            }
        });
        assert_eq!(*balance_sum.lock().unwrap(), Amount::new(4));
    }
}
