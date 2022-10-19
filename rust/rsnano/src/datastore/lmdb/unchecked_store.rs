use std::sync::Arc;

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    core::{HashOrAccount, UncheckedInfo, UncheckedKey},
    datastore::{lmdb::exists, unchecked_store::UncheckedIterator, UncheckedStore},
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbUncheckedStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbUncheckedStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("unchecked"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }
}

impl<'a> UncheckedStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbUncheckedStore
{
    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }

    fn put(
        &self,
        txn: &mut LmdbWriteTransaction,
        dependency: &HashOrAccount,
        info: &UncheckedInfo,
    ) {
        let key = UncheckedKey {
            previous: dependency.into(),
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
            .put(self.database, &key_bytes, &value_bytes, WriteFlags::empty())
            .unwrap();
    }

    fn exists(&self, txn: &LmdbTransaction, key: &UncheckedKey) -> bool {
        exists(txn, self.database, &key.to_bytes())
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, key: &UncheckedKey) {
        txn.rw_txn_mut()
            .del(self.database, &key.to_bytes(), None)
            .unwrap();
    }

    fn begin(&self, txn: &LmdbTransaction) -> UncheckedIterator<LmdbIteratorImpl> {
        UncheckedIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn lower_bound(
        &self,
        txn: &LmdbTransaction,
        key: &UncheckedKey,
    ) -> UncheckedIterator<LmdbIteratorImpl> {
        let key_bytes = key.to_bytes();
        UncheckedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.database,
            Some(&key_bytes),
            true,
        ))
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }
}
