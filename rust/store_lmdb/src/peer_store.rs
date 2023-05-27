use crate::{
    count, exists, iterator::DbIterator, EnvironmentStrategy, EnvironmentWrapper,
    LmdbEnv, LmdbIteratorImpl, Transaction, LmdbWriteTransaction,
};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use rsnano_core::{EndpointKey, NoValue};
use std::sync::Arc;

pub type PeerIterator = Box<dyn DbIterator<EndpointKey, NoValue>>;

pub struct LmdbPeerStore<T: EnvironmentStrategy = EnvironmentWrapper> {
    _env: Arc<LmdbEnv<T>>,
    database: Database,
}

impl<T: EnvironmentStrategy + 'static> LmdbPeerStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("peers"), DatabaseFlags::empty())?;

        Ok(Self {
            _env: env,
            database,
        })
    }

    pub fn database(&self) -> Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .put(
                self.database,
                &endpoint.to_bytes(),
                &[0; 0],
                WriteFlags::empty(),
            )
            .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .del(self.database, &endpoint.to_bytes(), None)
            .unwrap();
    }

    pub fn exists(&self, txn: &dyn Transaction, endpoint: &EndpointKey) -> bool {
        exists::<T>(txn, self.database, &endpoint.to_bytes())
    }

    pub fn count(&self, txn: &dyn Transaction) -> u64 {
        count::<T>(txn, self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }

    pub fn begin(&self, txn: &dyn Transaction) -> PeerIterator {
        LmdbIteratorImpl::new_iterator::<T, _, _>(txn, self.database, None, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;
    use rsnano_core::NoValue;

    use super::*;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert_eq!(store.count(&txn), 0);
        assert_eq!(store.exists(&txn, &test_endpoint_key()), false);
        assert!(store.begin(&txn).is_end());
        Ok(())
    }

    #[test]
    fn add_one_endpoint() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let key = test_endpoint_key();
        store.put(&mut txn, &key);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.exists(&txn, &key), true);
        assert_eq!(store.begin(&txn).current(), Some((&key, &NoValue {})));
        Ok(())
    }

    #[test]
    fn add_two_endpoints() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let key1 = test_endpoint_key();
        let key2 = EndpointKey::new([2; 16], 123);
        store.put(&mut txn, &key1);
        store.put(&mut txn, &key2);

        assert_eq!(store.count(&txn), 2);
        assert_eq!(store.exists(&txn, &key1), true);
        assert_eq!(store.exists(&txn, &key2), true);
        Ok(())
    }

    #[test]
    fn delete() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let key1 = test_endpoint_key();
        let key2 = EndpointKey::new([2; 16], 123);
        store.put(&mut txn, &key1);
        store.put(&mut txn, &key2);

        store.del(&mut txn, &key1);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.exists(&txn, &key1), false);
        assert_eq!(store.exists(&txn, &key2), true);
        Ok(())
    }

    fn test_endpoint_key() -> EndpointKey {
        EndpointKey::new([1; 16], 123)
    }
}
