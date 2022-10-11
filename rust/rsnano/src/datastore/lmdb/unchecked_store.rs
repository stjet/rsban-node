use std::sync::{Arc, Mutex};

use crate::{
    datastore::{unchecked_store::UncheckedIterator, UncheckedStore},
    unchecked_info::{UncheckedInfo, UncheckedKey},
    utils::Serialize,
    HashOrAccount,
};

use super::{
    assert_success, ensure_success, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_get, mdb_put,
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, OwnedMdbVal,
    Transaction, MDB_NOTFOUND, MDB_SUCCESS,
};

pub struct LmdbUncheckedStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbUncheckedStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(0),
        }
    }

    pub fn db_handle(&self) -> u32 {
        *self.db_handle.lock().unwrap()
    }

    pub fn open_db(&self, txn: &Transaction, flags: u32) -> anyhow::Result<()> {
        let mut handle = 0;
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("unchecked"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl<'a> UncheckedStore<'a, LmdbReadTransaction, LmdbWriteTransaction, LmdbIteratorImpl>
    for LmdbUncheckedStore
{
    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        let status = unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
        assert_success(status);
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
        let mut value = OwnedMdbVal::from(info);
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn exists(&self, txn: &Transaction, key: &UncheckedKey) -> bool {
        let mut value = MdbVal::new();
        let key_bytes = key.to_bytes();
        let status = unsafe {
            mdb_get(
                txn.handle(),
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                &mut value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        status == MDB_SUCCESS
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, key: &UncheckedKey) {
        let key_bytes = key.to_bytes();
        let status = unsafe {
            mdb_del(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                None,
            )
        };
        assert_success(status);
    }

    fn begin(&self, txn: &Transaction) -> UncheckedIterator<LmdbIteratorImpl> {
        UncheckedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            MdbVal::new(),
            UncheckedKey::serialized_size(),
            true,
        ))
    }

    fn lower_bound(
        &self,
        txn: &Transaction,
        key: &UncheckedKey,
    ) -> UncheckedIterator<LmdbIteratorImpl> {
        let key_bytes = key.to_bytes();
        UncheckedIterator::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            MdbVal::from_slice(&key_bytes),
            UncheckedKey::serialized_size(),
            true,
        ))
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }
}
