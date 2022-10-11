use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{peer_store::PeerIterator, PeerStore},
    EndpointKey,
};

use super::{
    exists, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbPeerStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbPeerStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(None),
        }
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle.lock().unwrap().unwrap()
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        let db = self
            .env
            .environment
            .create_db(Some("peers"), DatabaseFlags::empty())
            .unwrap();
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a> PeerStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbPeerStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .put(
                self.db_handle(),
                &endpoint.to_bytes(),
                &[0; 0],
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, endpoint: &EndpointKey) {
        txn.rw_txn_mut()
            .del(self.db_handle(), &endpoint.to_bytes(), None)
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, endpoint: &EndpointKey) -> bool {
        exists(txn, self.db_handle(), &endpoint.to_bytes())
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.db_handle())
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.db_handle()).unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> PeerIterator<LmdbIteratorImpl> {
        PeerIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }
}
