use std::sync::Arc;

use crate::{
    iterator::DbIterator, parallel_traversal_u512, Environment, EnvironmentWrapper, LmdbEnv,
    LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::{Deserialize, StreamAdapter},
    Account, BlockHash, PendingInfo, PendingKey,
};

pub type PendingIterator = Box<dyn DbIterator<PendingKey, PendingInfo>>;

pub struct LmdbPendingStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbPendingStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pending"), DatabaseFlags::empty())?;

        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, key: &PendingKey, pending: &PendingInfo) {
        let key_bytes = key.to_bytes();
        let pending_bytes = pending.to_bytes();
        txn.put(
            self.database,
            &key_bytes,
            &pending_bytes,
            WriteFlags::empty(),
        )
        .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, key: &PendingKey) {
        let key_bytes = key.to_bytes();
        txn.delete(self.database, &key_bytes, None).unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> Option<PendingInfo> {
        let key_bytes = key.to_bytes();
        match txn.get(self.database, &key_bytes) {
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                PendingInfo::deserialize(&mut stream).ok()
            }
            Err(lmdb::Error::NotFound) => None,
            Err(e) => {
                panic!("Could not load pending info: {:?}", e);
            }
        }
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> PendingIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_key(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> PendingIterator {
        let key_bytes = key.to_bytes();
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(&key_bytes), true)
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> bool {
        let iterator = self.begin_at_key(txn, key);
        iterator.current().map(|(k, _)| k == key).unwrap_or(false)
    }

    pub fn any(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> bool {
        let key = PendingKey::new(*account, BlockHash::zero());
        let iterator = self.begin_at_key(txn, &key);
        iterator
            .current()
            .map(|(k, _)| k.account == *account)
            .unwrap_or(false)
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, PendingIterator, PendingIterator) + Send + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_key(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_key(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    pub fn end(&self) -> PendingIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeleteEvent, EnvironmentStub, PutEvent};

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbPendingStore<EnvironmentStub>,
    }

    impl Fixture {
        pub fn new() -> Self {
            Self::with_stored_data(Vec::new())
        }

        pub fn with_stored_data(entries: Vec<(PendingKey, PendingInfo)>) -> Self {
            let mut env = LmdbEnv::create_null_with().database("pending", Default::default());

            for (key, value) in entries {
                env = env.entry(&key.to_bytes(), &value.to_bytes())
            }

            let env = Arc::new(env.build().build());
            Self {
                env: env.clone(),
                store: LmdbPendingStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn not_found() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        let result = fixture.store.get(&txn, &PendingKey::create_test_instance());
        assert!(result.is_none());
        assert_eq!(
            fixture
                .store
                .exists(&txn, &PendingKey::create_test_instance()),
            false
        );
    }

    #[test]
    fn load_pending_info() {
        let key = PendingKey::create_test_instance();
        let info = PendingInfo::create_test_instance();
        let fixture = Fixture::with_stored_data(vec![(key.clone(), info.clone())]);
        let txn = fixture.env.tx_begin_read();

        let result = fixture.store.get(&txn, &key);

        assert_eq!(result, Some(info));
        assert_eq!(fixture.store.exists(&txn, &key), true);
    }

    #[test]
    fn add_pending() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();
        let pending_key = PendingKey::create_test_instance();
        let pending = PendingInfo::create_test_instance();

        fixture.store.put(&mut txn, &pending_key, &pending);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: Default::default(),
                key: pending_key.to_bytes().to_vec(),
                value: pending.to_bytes().to_vec(),
                flags: WriteFlags::empty()
            }]
        );
    }

    #[test]
    fn delete() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();
        let pending_key = PendingKey::create_test_instance();

        fixture.store.del(&mut txn, &pending_key);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                database: Default::default(),
                key: pending_key.to_bytes().to_vec()
            }]
        )
    }

    #[test]
    fn iter_empty() {
        let fixture = Fixture::new();
        let txn = fixture.env.tx_begin_read();
        assert!(fixture.store.begin(&txn).is_end());
    }

    #[test]
    fn iter() {
        let key = PendingKey::create_test_instance();
        let info = PendingInfo::create_test_instance();
        let fixture = Fixture::with_stored_data(vec![(key.clone(), info.clone())]);
        let txn = fixture.env.tx_begin_read();

        let mut it = fixture.store.begin(&txn);
        assert_eq!(it.is_end(), false);
        let (k, v) = it.current().unwrap();
        assert_eq!(k, &key);
        assert_eq!(v, &info);

        it.next();
        assert!(it.is_end());
    }
}
