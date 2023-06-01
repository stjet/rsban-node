use crate::{
    iterator::DbIterator, lmdb_env::EnvironmentWrapper, parallel_traversal, Environment, LmdbEnv,
    LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::{Deserialize, OutputListenerMt, OutputTrackerMt, StreamAdapter},
    Account, AccountInfo,
};
use std::sync::Arc;

pub type AccountIterator = Box<dyn DbIterator<Account, AccountInfo>>;

pub struct LmdbAccountStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,

    /// U256 (arbitrary key) -> blob
    database: T::Database,
    #[cfg(feature = "output_tracking")]
    put_listener: OutputListenerMt<(Account, AccountInfo)>,
}

impl<T: Environment + 'static> LmdbAccountStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("accounts"), DatabaseFlags::empty())?;
        Ok(Self {
            env,
            database,
            #[cfg(feature = "output_tracking")]
            put_listener: OutputListenerMt::new(),
        })
    }

    pub fn track_puts(&self) -> Arc<OutputTrackerMt<(Account, AccountInfo)>> {
        self.put_listener.track()
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(
        &self,
        transaction: &mut LmdbWriteTransaction<T>,
        account: &Account,
        info: &AccountInfo,
    ) {
        #[cfg(feature = "output_tracking")]
        self.put_listener.emit((account.clone(), info.clone()));
        transaction
            .put(
                self.database,
                account.as_bytes(),
                &info.to_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    pub fn get(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Option<AccountInfo> {
        let result = transaction.get(self.database, account.as_bytes());
        match result {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                AccountInfo::deserialize(&mut stream).ok()
            }
            Err(e) => panic!("Could not load account info {:?}", e),
        }
    }

    pub fn del(&self, transaction: &mut LmdbWriteTransaction<T>, account: &Account) {
        transaction
            .delete(self.database, account.as_bytes(), None)
            .unwrap();
    }

    pub fn begin_account(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> AccountIterator {
        LmdbIteratorImpl::<T>::new_iterator(
            transaction,
            self.database,
            Some(account.as_bytes()),
            true,
        )
    }

    pub fn begin(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> AccountIterator {
        LmdbIteratorImpl::<T>::new_iterator(transaction, self.database, None, true)
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, AccountIterator, AccountIterator) + Send + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let txn = self.env.tx_begin_read();
            let begin_it = self.begin_account(&txn, &start.into());
            let end_it = if !is_last {
                self.begin_account(&txn, &end.into())
            } else {
                self.end()
            };
            action(&txn, begin_it, end_it);
        })
    }

    pub fn end(&self) -> AccountIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> bool {
        !self.begin_account(txn, account).is_end()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{lmdb_env::DatabaseStub, DeleteEvent, EnvironmentStub, PutEvent};
    use rsnano_core::{Amount, BlockHash};

    use super::*;

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbAccountStore<EnvironmentStub>,
    }

    impl Fixture {
        fn new() -> Self {
            let env = Arc::new(LmdbEnv::create_null());
            let store = LmdbAccountStore::new(env.clone()).unwrap();

            Self { env, store }
        }

        fn with_env(env: LmdbEnv<EnvironmentStub>) -> Self {
            let env = Arc::new(env);
            let store = LmdbAccountStore::new(env.clone()).unwrap();

            Fixture { env, store }
        }
    }

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let account = Account::from(1);
        let result = fixture.store.get(&txn, &account);
        assert_eq!(result, None);
        assert_eq!(fixture.store.exists(&txn, &account), false);
        assert_eq!(fixture.store.count(&txn), 0);
    }

    #[test]
    fn add_one_account() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        let account = Account::from(1);
        let info = AccountInfo::create_test_instance();
        fixture.store.put(&mut txn, &account, &info);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: Default::default(),
                key: account.as_bytes().to_vec(),
                value: info.to_bytes().to_vec(),
                flags: lmdb::WriteFlags::empty()
            }]
        );
    }

    #[test]
    fn load_account() {
        let account = Account::from(1);
        let info = AccountInfo::create_test_instance();
        let env = LmdbEnv::create_null_with()
            .database("accounts", DatabaseStub(1))
            .entry(account.as_bytes(), &info.to_bytes())
            .build()
            .build();
        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let result = fixture.store.get(&txn, &account);

        assert_eq!(result, Some(info));
    }

    #[test]
    fn count() {
        let info = AccountInfo::create_test_instance();
        let env = LmdbEnv::create_null_with()
            .database("accounts", DatabaseStub(1))
            .entry(Account::from(1).as_bytes(), &info.to_bytes())
            .entry(Account::from(2).as_bytes(), &info.to_bytes())
            .build()
            .build();
        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let count = fixture.store.count(&txn);

        assert_eq!(count, 2);
    }

    #[test]
    fn delete_account() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        let account = Account::from(1);
        fixture.store.del(&mut txn, &account);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: Default::default(),
                key: account.as_bytes().to_vec()
            }]
        )
    }

    #[test]
    fn begin_empty_store_nullable() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let it = fixture.store.begin(&txn);
        assert_eq!(it.is_end(), true);
    }

    #[test]
    fn begin() {
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

        let env = LmdbEnv::create_null_with()
            .database("accounts", DatabaseStub(42))
            .entry(account1.as_bytes(), &info1.to_bytes())
            .entry(account2.as_bytes(), &info2.to_bytes())
            .build()
            .build();

        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let mut it = fixture.store.begin(&txn);
        assert_eq!(it.current(), Some((&account1, &info1)));
        it.next();
        assert_eq!(it.current(), Some((&account2, &info2)));
        it.next();
        assert_eq!(it.current(), None);
    }

    #[test]
    fn begin_account() {
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

        let env = LmdbEnv::create_null_with()
            .database("accounts", DatabaseStub(42))
            .entry(account1.as_bytes(), &info1.to_bytes())
            .entry(account3.as_bytes(), &info3.to_bytes())
            .build()
            .build();

        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let mut it = fixture.store.begin_account(&txn, &Account::from(2));

        assert_eq!(it.current(), Some((&account3, &info3)));
        it.next();
        assert_eq!(it.current(), None);
    }

    #[test]
    fn for_each_par() {
        let account1 = Account::from(1);
        let account3 = Account::from(3);
        let info1 = AccountInfo {
            balance: Amount::raw(1),
            ..Default::default()
        };
        let info3 = AccountInfo {
            balance: Amount::raw(3),
            ..Default::default()
        };

        let env = LmdbEnv::create_null_with()
            .database("accounts", DatabaseStub(42))
            .entry(account1.as_bytes(), &info1.to_bytes())
            .entry(account3.as_bytes(), &info3.to_bytes())
            .build()
            .build();

        let fixture = Fixture::with_env(env);

        let balance_sum = Mutex::new(Amount::zero());
        fixture.store.for_each_par(&|_, mut begin, end| {
            while !begin.eq(end.as_ref()) {
                if let Some((_, v)) = begin.current() {
                    *balance_sum.lock().unwrap() += v.balance
                }
                begin.next();
            }
        });
        assert_eq!(*balance_sum.lock().unwrap(), Amount::raw(4));
    }

    #[test]
    fn track_inserted_account_info() {
        let fixture = Fixture::new();
        let put_tracker = fixture.store.track_puts();
        let mut txn = fixture.env.tx_begin_write();
        let account = Account::from(1);
        let info = AccountInfo::create_test_instance();

        fixture.store.put(&mut txn, &account, &info);

        assert_eq!(put_tracker.output(), vec![(account, info)]);
    }
}
