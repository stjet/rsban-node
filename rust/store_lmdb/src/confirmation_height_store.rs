use crate::{
    iterator::DbIterator, lmdb_env::RwTransaction, parallel_traversal, Environment,
    EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction,
    Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::{Deserialize, StreamAdapter},
    Account, ConfirmationHeightInfo,
};
use std::sync::Arc;

pub type ConfirmationHeightIterator = Box<dyn DbIterator<Account, ConfirmationHeightInfo>>;

pub struct LmdbConfirmationHeightStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbConfirmationHeightStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("confirmation_height"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        account: &Account,
        info: &ConfirmationHeightInfo,
    ) {
        txn.put(
            self.database,
            account.as_bytes(),
            &info.to_bytes(),
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Option<ConfirmationHeightInfo> {
        match txn.get(self.database, account.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                ConfirmationHeightInfo::deserialize(&mut stream).ok()
            }
            Err(e) => {
                panic!("Could not load confirmation height info: {:?}", e);
            }
        }
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> bool {
        txn.exists(self.database, account.as_bytes())
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, account: &Account) {
        txn.delete(self.database, account.as_bytes(), None).unwrap();
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.clear_db(self.database).unwrap()
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_account(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(account.as_bytes()), true)
    }

    pub fn end(&self) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, ConfirmationHeightIterator, ConfirmationHeightIterator)
              + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_account(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_account(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::{lmdb_env::DatabaseStub, EnvironmentStub, PutEvent, TestLmdbEnv};
    use rsnano_core::BlockHash;

    use super::*;

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbConfirmationHeightStore<EnvironmentStub>,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_env(LmdbEnv::create_null())
        }

        fn with_env(env: LmdbEnv<EnvironmentStub>) -> Self {
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbConfirmationHeightStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let store = &fixture.store;
        let txn = fixture.env.tx_begin_read();
        assert!(store.get(&txn, &Account::from(0)).is_none());
        assert_eq!(store.exists(&txn, &Account::from(0)), false);
        assert!(store.begin(&txn).is_end());
        assert!(store.begin_at_account(&txn, &Account::from(0)).is_end());
    }

    #[test]
    fn add_account() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        let account = Account::from(1);
        let info = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        fixture.store.put(&mut txn, &account, &info);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: Default::default(),
                key: account.as_bytes().to_vec(),
                value: info.to_bytes().to_vec(),
                flags: WriteFlags::empty(),
            }]
        )
    }

    #[test]
    fn load() {
        let account = Account::from(1);
        let info = ConfirmationHeightInfo::new(1, BlockHash::from(2));

        let env = LmdbEnv::create_null_with()
            .database("confirmation_height", DatabaseStub(100))
            .entry(account.as_bytes(), &info.to_bytes())
            .build()
            .build();

        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();
        let result = fixture.store.get(&txn, &account);

        assert_eq!(result, Some(info))
    }

    #[test]
    fn iterate_one_account() -> anyhow::Result<()> {
        let account = Account::from(1);
        let info = ConfirmationHeightInfo::new(1, BlockHash::from(2));

        let env = LmdbEnv::create_null_with()
            .database("confirmation_height", DatabaseStub(100))
            .entry(account.as_bytes(), &info.to_bytes())
            .build()
            .build();

        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();
        let mut it = fixture.store.begin(&txn);
        assert_eq!(it.current(), Some((&account, &info)));

        it.next();
        assert!(it.is_end());
        Ok(())
    }

    #[test]
    fn clear() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write();
        let account1 = Account::from(1);
        let account2 = Account::from(2);
        let info1 = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        let info2 = ConfirmationHeightInfo::new(3, BlockHash::from(4));
        store.put(&mut txn, &account1, &info1);
        store.put(&mut txn, &account2, &info2);

        store.clear(&mut txn);

        assert_eq!(store.count(&txn), 0);
        Ok(())
    }
}
