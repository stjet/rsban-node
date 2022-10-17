use lmdb::{Database, DatabaseFlags, WriteFlags};
use std::sync::Arc;

use crate::{
    datastore::{peer_store::PeerIterator, PeerStore},
    EndpointKey,
};

use super::{
    exists, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbPeerStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbPeerStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("peers"), DatabaseFlags::empty())?;

        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }
}

impl<'a> PeerStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbPeerStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .put(
                self.database,
                &endpoint.to_bytes(),
                &[0; 0],
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .del(self.database, &endpoint.to_bytes(), None)
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, endpoint: &EndpointKey) -> bool {
        exists(txn, self.database, &endpoint.to_bytes())
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> PeerIterator<LmdbIteratorImpl> {
        PeerIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }
}

#[cfg(test)]
mod tests {
    use crate::{datastore::lmdb::TestLmdbEnv, NoValue};

    use super::*;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert_eq!(store.count(&txn.as_txn()), 0);
        assert_eq!(store.exists(&txn.as_txn(), &test_endpoint_key()), false);
        assert!(store.begin(&txn.as_txn()).is_end());
        Ok(())
    }

    #[test]
    fn add_one_endpoint() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPeerStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let key = test_endpoint_key();
        store.put(&mut txn, &key);

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(store.exists(&txn.as_txn(), &key), true);
        assert_eq!(
            store.begin(&txn.as_txn()).current(),
            Some((&key, &NoValue {}))
        );
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

        assert_eq!(store.count(&txn.as_txn()), 2);
        assert_eq!(store.exists(&txn.as_txn(), &key1), true);
        assert_eq!(store.exists(&txn.as_txn(), &key2), true);
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

        assert_eq!(store.count(&txn.as_txn()), 1);
        assert_eq!(store.exists(&txn.as_txn(), &key1), false);
        assert_eq!(store.exists(&txn.as_txn(), &key2), true);
        Ok(())
    }

    fn test_endpoint_key() -> EndpointKey {
        EndpointKey::new([1; 16], 123)
    }
}
