use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{lmdb::exists, unchecked_store::UncheckedIterator, UncheckedStore},
    unchecked_info::{UncheckedInfo, UncheckedKey},
    HashOrAccount,
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbUncheckedStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbUncheckedStore {
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
            .create_db(Some("unchecked"), DatabaseFlags::empty())
            .unwrap();
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a> UncheckedStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbUncheckedStore
{
    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.db_handle()).unwrap();
    }

    fn put(
        &self,
        txn: &mut LmdbWriteTransaction,
        dependency: &HashOrAccount,
        info: &UncheckedInfo,
    ) {
        let key = UncheckedKey {
            previous: dependency.to_block_hash(),
            hash: info
                .block
                .as_ref()
                .unwrap()
                .read()
                .unwrap()
                .as_block()
                .hash(),
        };
        let key_bytes = key.to_bytes();
        let value_bytes = info.to_bytes();
        txn.rw_txn_mut()
            .put(
                self.db_handle(),
                &key_bytes,
                &value_bytes,
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, key: &UncheckedKey) -> bool {
        exists(txn, self.db_handle(), &key.to_bytes())
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, key: &UncheckedKey) {
        txn.rw_txn_mut()
            .del(self.db_handle(), &key.to_bytes(), None)
            .unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> UncheckedIterator<LmdbIteratorImpl> {
        UncheckedIterator::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }

    fn lower_bound(
        &self,
        txn: &LmdbTransaction,
        key: &UncheckedKey,
    ) -> UncheckedIterator<LmdbIteratorImpl> {
        let key_bytes = key.to_bytes();
        UncheckedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            Some(&key_bytes),
            true,
        ))
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.db_handle())
    }
}
