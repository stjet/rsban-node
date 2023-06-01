use crate::{
    iterator::DbIterator, Environment, EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl,
    LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{EndpointKey, NoValue};
use std::sync::Arc;

pub type PeerIterator = Box<dyn DbIterator<EndpointKey, NoValue>>;

pub struct LmdbPeerStore<T: Environment = EnvironmentWrapper> {
    _env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbPeerStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("peers"), DatabaseFlags::empty())?;

        Ok(Self {
            _env: env,
            database,
        })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, endpoint: &EndpointKey) {
        txn.put(
            self.database,
            &endpoint.to_bytes(),
            &[0; 0],
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, endpoint: &EndpointKey) {
        txn.delete(self.database, &endpoint.to_bytes(), None)
            .unwrap();
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        endpoint: &EndpointKey,
    ) -> bool {
        txn.exists(self.database, &endpoint.to_bytes())
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

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> PeerIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lmdb_env::DatabaseStub, DeleteEvent, EnvironmentStub, PutEvent};
    use rsnano_core::NoValue;

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbPeerStore<EnvironmentStub>,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_env(LmdbEnv::create_null())
        }

        fn with_stored_data(entries: Vec<EndpointKey>) -> Self {
            let mut env = LmdbEnv::create_null_with().database("peers", DatabaseStub::default());

            for entry in entries {
                env = env.entry(&entry.to_bytes(), &[]);
            }

            Self::with_env(env.build().build())
        }

        fn with_env(env: LmdbEnv<EnvironmentStub>) -> Self {
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbPeerStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn empty_store() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let store = &fixture.store;
        assert_eq!(store.count(&txn), 0);
        assert_eq!(
            store.exists(&txn, &EndpointKey::create_test_instance()),
            false
        );
        assert!(store.begin(&txn).is_end());
    }

    #[test]
    fn add_one_endpoint() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        let key = EndpointKey::create_test_instance();
        fixture.store.put(&mut txn, &key);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: Default::default(),
                key: key.to_bytes().to_vec(),
                value: Vec::new(),
                flags: WriteFlags::empty()
            }]
        )
    }

    #[test]
    fn exists() {
        let endpoint_a = EndpointKey::new([1; 16], 1000);
        let endpoint_b = EndpointKey::new([2; 16], 2000);
        let unknown_endpoint = EndpointKey::new([3; 16], 3000);
        let fixture = Fixture::with_stored_data(vec![endpoint_a.clone(), endpoint_b.clone()]);

        let txn = fixture.env.tx_begin_read();

        assert_eq!(fixture.store.exists(&txn, &endpoint_a), true);
        assert_eq!(fixture.store.exists(&txn, &endpoint_b), true);
        assert_eq!(fixture.store.exists(&txn, &unknown_endpoint), false);
    }

    #[test]
    fn count() {
        let endpoint_a = EndpointKey::new([1; 16], 1000);
        let endpoint_b = EndpointKey::new([2; 16], 2000);
        let fixture = Fixture::with_stored_data(vec![endpoint_a, endpoint_b]);

        let txn = fixture.env.tx_begin_read();

        assert_eq!(fixture.store.count(&txn), 2);
    }

    #[test]
    fn iterate() {
        let endpoint_a = EndpointKey::new([1; 16], 1000);
        let endpoint_b = EndpointKey::new([2; 16], 2000);
        let fixture = Fixture::with_stored_data(vec![endpoint_a.clone(), endpoint_b.clone()]);

        let txn = fixture.env.tx_begin_read();
        let mut it = fixture.store.begin(&txn);
        assert_eq!(it.current(), Some((&endpoint_a, &NoValue {})));
        it.next();
        assert_eq!(it.current(), Some((&endpoint_b, &NoValue {})));
        it.next();
        assert_eq!(it.current(), None);
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        let key = EndpointKey::create_test_instance();
        fixture.store.del(&mut txn, &key);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: Default::default(),
                key: key.to_bytes().to_vec()
            }]
        )
    }
}
