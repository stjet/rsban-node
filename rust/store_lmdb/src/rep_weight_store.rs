use crate::{
    lmdb_env::RoCursor, ConfiguredDatabase, Environment, EnvironmentWrapper, LmdbEnv,
    LmdbWriteTransaction, Transaction, REP_WEIGHT_TEST_DATABASE,
};
use lmdb::{DatabaseFlags, WriteFlags};
use lmdb_sys::{MDB_cursor_op, MDB_FIRST, MDB_NEXT};
#[cfg(feature = "output_tracking")]
use rsnano_core::utils::{OutputListenerMt, OutputTrackerMt};
use rsnano_core::{
    utils::{BufferReader, Deserialize},
    Account, Amount,
};
use std::{marker::PhantomData, sync::Arc};

pub struct LmdbRepWeightStore<T: Environment = EnvironmentWrapper> {
    _env: Arc<LmdbEnv<T>>,
    database: T::Database,
    #[cfg(feature = "output_tracking")]
    delete_listener: OutputListenerMt<Account>,
}

impl<T: Environment + 'static> LmdbRepWeightStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("rep_weights"), DatabaseFlags::empty())?;
        Ok(Self {
            _env: env,
            database,
            #[cfg(feature = "output_tracking")]
            delete_listener: OutputListenerMt::new(),
        })
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_deletions(&self) -> Arc<OutputTrackerMt<Account>> {
        self.delete_listener.track()
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: Account,
    ) -> Option<Amount> {
        match txn.get(self.database, account.as_bytes()) {
            Ok(bytes) => {
                let mut stream = BufferReader::new(bytes);
                Amount::deserialize(&mut stream).ok()
            }
            Err(lmdb::Error::NotFound) => None,
            Err(e) => {
                panic!("Could not load rep_weight: {:?}", e);
            }
        }
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, account: Account, weight: Amount) {
        txn.put(
            self.database,
            account.as_bytes(),
            &weight.to_be_bytes(),
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, account: Account) {
        #[cfg(feature = "output_tracking")]
        self.delete_listener.emit(account);

        txn.delete(self.database, account.as_bytes(), None).unwrap();
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn iter<'a>(
        &self,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> RepWeightIterator<'a, T> {
        let cursor = txn.open_ro_cursor(self.database).unwrap();
        RepWeightIterator {
            cursor,
            _lifetime: Default::default(),
            operation: MDB_FIRST,
        }
    }
}

pub struct RepWeightIterator<'a, T: Environment + 'static> {
    _lifetime: PhantomData<&'a ()>,
    cursor: T::RoCursor,
    operation: MDB_cursor_op,
}

impl<'a, T: Environment + 'static> Iterator for RepWeightIterator<'a, T> {
    type Item = (Account, Amount);

    fn next(&mut self) -> Option<Self::Item> {
        match self.cursor.get(None, None, self.operation) {
            Err(lmdb::Error::NotFound) => None,
            Ok((Some(k), v)) => {
                self.operation = MDB_NEXT;
                Some((
                    Account::from_slice(k).unwrap(),
                    Amount::from_be_bytes(v.try_into().unwrap()),
                ))
            }
            Ok(_) => unreachable!(),
            Err(_) => unreachable!(),
        }
    }
}

pub struct ConfiguredRepWeightDatabaseBuilder {
    database: ConfiguredDatabase,
}

impl ConfiguredRepWeightDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            database: ConfiguredDatabase::new(REP_WEIGHT_TEST_DATABASE, "rep_weights"),
        }
    }

    pub fn entry(mut self, account: Account, weight: Amount) -> Self {
        self.database
            .entries
            .insert(account.as_bytes().to_vec(), weight.to_be_bytes().to_vec());
        self
    }

    pub fn build(self) -> ConfiguredDatabase {
        self.database
    }

    pub fn create(hashes: Vec<(Account, Amount)>) -> ConfiguredDatabase {
        let mut builder = Self::new();
        for (account, weight) in hashes {
            builder = builder.entry(account, weight);
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use lmdb::WriteFlags;

    use super::*;
    use crate::{DeleteEvent, EnvironmentStub, LmdbEnv, PutEvent};

    #[test]
    fn count() {
        let fixture =
            Fixture::with_stored_data(vec![(1.into(), 100.into()), (2.into(), 200.into())]);
        let txn = fixture.env.tx_begin_read();

        assert_eq!(fixture.store.count(&txn), 2);
    }

    #[test]
    fn put() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();
        let account = Account::from(1);
        let weight = Amount::from(42);

        fixture.store.put(&mut txn, account, weight);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: REP_WEIGHT_TEST_DATABASE,
                key: account.as_bytes().to_vec(),
                value: weight.to_be_bytes().to_vec(),
                flags: WriteFlags::empty()
            }]
        );
    }

    #[test]
    fn load_weight() {
        let account = Account::from(1);
        let weight = Amount::from(42);
        let fixture = Fixture::with_stored_data(vec![(account, weight)]);
        let txn = fixture.env.tx_begin_read();

        let result = fixture.store.get(&txn, account);

        assert_eq!(result, Some(weight));
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();
        let account = Account::from(1);

        fixture.store.del(&mut txn, account);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: REP_WEIGHT_TEST_DATABASE,
                key: account.as_bytes().to_vec()
            }]
        )
    }

    #[test]
    fn iter_empty() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let mut iter = fixture.store.iter(&txn);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter() {
        let account1 = Account::from(1);
        let account2 = Account::from(2);
        let weight1 = Amount::from(100);
        let weight2 = Amount::from(200);
        let fixture = Fixture::with_stored_data(vec![(account1, weight1), (account2, weight2)]);

        let txn = fixture.env.tx_begin_read();
        let mut iter = fixture.store.iter(&txn);
        assert_eq!(iter.next(), Some((account1, weight1)));
        assert_eq!(iter.next(), Some((account2, weight2)));
        assert_eq!(iter.next(), None);
    }

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbRepWeightStore<EnvironmentStub>,
    }

    impl Fixture {
        pub fn new() -> Self {
            Self::with_stored_data(Vec::new())
        }

        pub fn with_stored_data(entries: Vec<(Account, Amount)>) -> Self {
            let env = LmdbEnv::create_null_with()
                .configured_database(ConfiguredRepWeightDatabaseBuilder::create(entries))
                .build();
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbRepWeightStore::new(env).unwrap(),
            }
        }
    }
}
