use crate::FRONTIER_TEST_DATABASE;
use crate::{
    iterator::DbIterator, parallel_traversal, ConfiguredDatabase, Environment, EnvironmentWrapper,
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::utils::{OutputListenerMt, OutputTrackerMt};
use rsnano_core::{Account, BlockHash};
use std::sync::Arc;

pub type FrontierIterator = Box<dyn DbIterator<BlockHash, Account>>;

pub struct ConfiguredFrontierDatabaseBuilder {
    database: ConfiguredDatabase,
}

impl ConfiguredFrontierDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            database: ConfiguredDatabase::new(FRONTIER_TEST_DATABASE, "frontiers"),
        }
    }

    pub fn frontier(mut self, hash: &BlockHash, account: &Account) -> Self {
        self.database
            .entries
            .insert(hash.as_bytes().to_vec(), account.as_bytes().to_vec());
        self
    }

    pub fn build(self) -> ConfiguredDatabase {
        self.database
    }

    pub fn create(frontiers: Vec<(BlockHash, Account)>) -> ConfiguredDatabase {
        let mut builder = Self::new();
        for (hash, account) in frontiers {
            builder = builder.frontier(&hash, &account);
        }
        builder.build()
    }
}

pub struct LmdbFrontierStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
    #[cfg(feature = "output_tracking")]
    put_listener: OutputListenerMt<(BlockHash, Account)>,
    #[cfg(feature = "output_tracking")]
    delete_listener: OutputListenerMt<BlockHash>,
}

impl<T: Environment + 'static> LmdbFrontierStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("frontiers"), DatabaseFlags::empty())?;
        Ok(Self {
            env,
            database,
            #[cfg(feature = "output_tracking")]
            put_listener: OutputListenerMt::new(),
            #[cfg(feature = "output_tracking")]
            delete_listener: OutputListenerMt::new(),
        })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_puts(&self) -> Arc<OutputTrackerMt<(BlockHash, Account)>> {
        self.put_listener.track()
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_deletions(&self) -> Arc<OutputTrackerMt<BlockHash>> {
        self.delete_listener.track()
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash, account: &Account) {
        #[cfg(feature = "output_tracking")]
        self.put_listener.emit((hash.clone(), account.clone()));
        txn.put(
            self.database,
            hash.as_bytes(),
            account.as_bytes(),
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Account> {
        match txn.get(self.database, hash.as_bytes()) {
            Ok(bytes) => Some(Account::from_slice(bytes).unwrap()),
            Err(lmdb::Error::NotFound) => None,
            Err(e) => panic!("Could not load frontier: {:?}", e),
        }
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        #[cfg(feature = "output_tracking")]
        self.delete_listener.emit(hash.clone());
        txn.delete(self.database, hash.as_bytes(), None).unwrap();
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> FrontierIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> FrontierIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(hash.as_bytes()), true)
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, FrontierIterator, FrontierIterator)
              + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_hash(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    pub fn end(&self) -> FrontierIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeleteEvent, EnvironmentStub, PutEvent};

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbFrontierStore<EnvironmentStub>,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_stored_frontieres(Vec::new())
        }

        fn with_stored_frontieres(frontiers: Vec<(BlockHash, Account)>) -> Self {
            let env = LmdbEnv::create_null_with()
                .configured_database(ConfiguredFrontierDatabaseBuilder::create(frontiers))
                .build();

            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbFrontierStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        assert_eq!(fixture.store.get(&txn, &BlockHash::from(1)), None);
        assert!(fixture.store.begin(&txn).is_end());
    }

    #[test]
    fn get_frontier() {
        let fixture = Fixture::with_stored_frontieres(vec![(BlockHash::from(1), Account::from(2))]);
        let txn = fixture.env.tx_begin_read();
        assert_eq!(
            fixture.store.get(&txn, &BlockHash::from(1)),
            Some(Account::from(2))
        );
    }

    #[test]
    fn put() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        fixture
            .store
            .put(&mut txn, &BlockHash::from(1), &Account::from(2));

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: FRONTIER_TEST_DATABASE,
                key: BlockHash::from(1).as_bytes().to_vec(),
                value: Account::from(2).as_bytes().to_vec(),
                flags: WriteFlags::empty(),
            }]
        );
    }

    #[test]
    fn can_track_puts() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = fixture.store.track_puts();

        fixture
            .store
            .put(&mut txn, &BlockHash::from(1), &Account::from(2));

        assert_eq!(
            put_tracker.output(),
            vec![(BlockHash::from(1), Account::from(2))]
        );
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        fixture.store.del(&mut txn, &BlockHash::from(42));

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: FRONTIER_TEST_DATABASE,
                key: BlockHash::from(42).as_bytes().to_vec()
            }]
        );
    }

    #[test]
    fn can_track_deletions() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = fixture.store.track_deletions();

        fixture.store.del(&mut txn, &BlockHash::from(42));

        assert_eq!(delete_tracker.output(), vec![BlockHash::from(42)]);
    }

    #[test]
    fn can_be_nulled() {
        let hash = BlockHash::from(1);
        let account = Account::from(2);
        let env = LmdbEnv::create_null_with()
            .configured_database(
                ConfiguredFrontierDatabaseBuilder::new()
                    .frontier(&hash, &account)
                    .build(),
            )
            .build();
        let txn = env.tx_begin_read();
        let store = LmdbFrontierStore::new(Arc::new(env)).unwrap();

        assert_eq!(store.get(&txn, &hash), Some(account));
    }
}
