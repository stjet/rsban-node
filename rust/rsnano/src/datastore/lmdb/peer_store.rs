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
