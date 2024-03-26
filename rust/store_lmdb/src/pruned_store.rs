use crate::{
    iterator::DbIterator, lmdb_env::EnvironmentWrapper, parallel_traversal, ConfiguredDatabase,
    Environment, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, Transaction,
    PRUNED_TEST_DATABASE,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rand::{thread_rng, Rng};
use rsnano_core::{BlockHash, NoValue};
use std::sync::Arc;

pub type PrunedIterator = Box<dyn DbIterator<BlockHash, NoValue>>;

pub struct LmdbPrunedStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbPrunedStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pruned"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        txn.put(self.database, hash.as_bytes(), &[0; 0], WriteFlags::empty())
            .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        txn.delete(self.database, hash.as_bytes(), None).unwrap();
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> bool {
        txn.exists(self.database, hash.as_bytes())
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> PrunedIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> PrunedIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(hash.as_bytes()), true)
    }

    pub fn random(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Option<BlockHash> {
        let random_hash = BlockHash::from_bytes(thread_rng().gen());
        let mut existing = self.begin_at_hash(txn, &random_hash);
        if existing.is_end() {
            existing = self.begin(txn);
        }

        existing.current().map(|(k, _)| *k)
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.clear_db(self.database).unwrap();
    }

    pub fn end(&self) -> PrunedIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, PrunedIterator, PrunedIterator) + Send + Sync),
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
}

pub struct ConfiguredPrunedDatabaseBuilder {
    database: ConfiguredDatabase,
}

impl ConfiguredPrunedDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            database: ConfiguredDatabase::new(PRUNED_TEST_DATABASE, "pruned"),
        }
    }

    pub fn pruned(mut self, hash: &BlockHash) -> Self {
        self.database
            .entries
            .insert(hash.as_bytes().to_vec(), Vec::new());
        self
    }

    pub fn build(self) -> ConfiguredDatabase {
        self.database
    }

    pub fn create(hashes: Vec<BlockHash>) -> ConfiguredDatabase {
        let mut builder = Self::new();
        for hash in hashes {
            builder = builder.pruned(&hash);
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeleteEvent, EnvironmentStub, PutEvent};

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbPrunedStore<EnvironmentStub>,
    }

    impl Fixture {
        pub fn new() -> Self {
            Self::with_stored_data(Vec::new())
        }

        pub fn with_stored_data(entries: Vec<BlockHash>) -> Self {
            let env = LmdbEnv::create_null_with()
                .configured_database(ConfiguredPrunedDatabaseBuilder::create(entries))
                .build();
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbPrunedStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let store = &fixture.store;

        assert_eq!(store.count(&txn), 0);
        assert_eq!(store.exists(&txn, &BlockHash::from(1)), false);
        assert_eq!(store.begin(&txn).is_end(), true);
        assert!(store.random(&txn).is_none());
    }

    #[test]
    fn add_pruned_info() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();
        let hash = BlockHash::from(1);

        fixture.store.put(&mut txn, &hash);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: PRUNED_TEST_DATABASE,
                key: hash.as_bytes().to_vec(),
                value: Vec::new(),
                flags: WriteFlags::empty()
            }]
        );
    }

    #[test]
    fn count() {
        let fixture = Fixture::with_stored_data(vec![BlockHash::from(1), BlockHash::from(2)]);
        let txn = fixture.env.tx_begin_read();

        assert_eq!(fixture.store.count(&txn), 2);
        assert_eq!(fixture.store.exists(&txn, &BlockHash::from(1)), true);
        assert_eq!(fixture.store.exists(&txn, &BlockHash::from(3)), false);
    }

    #[test]
    fn iterate() {
        let fixture = Fixture::with_stored_data(vec![BlockHash::from(1), BlockHash::from(2)]);
        let txn = fixture.env.tx_begin_read();

        assert_eq!(
            fixture.store.begin(&txn).current(),
            Some((&BlockHash::from(1), &NoValue {}))
        );
        assert_eq!(
            fixture
                .store
                .begin_at_hash(&txn, &BlockHash::from(2))
                .current(),
            Some((&BlockHash::from(2), &NoValue {}))
        );
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();
        let hash = BlockHash::from(1);

        fixture.store.del(&mut txn, &hash);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: PRUNED_TEST_DATABASE,
                key: hash.as_bytes().to_vec()
            }]
        )
    }

    #[test]
    fn pruned_random() {
        let fixture = Fixture::with_stored_data(vec![BlockHash::from(42)]);
        let txn = fixture.env.tx_begin_read();
        let random_hash = fixture.store.random(&txn);
        assert_eq!(random_hash, Some(BlockHash::from(42)));
    }
}
