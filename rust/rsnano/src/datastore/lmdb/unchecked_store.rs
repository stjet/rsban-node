use std::sync::Arc;

use crate::{
    datastore::{DbIterator, Transaction, UncheckedStore, WriteTransaction},
    unchecked_info::{UncheckedInfo, UncheckedKey},
    HashOrAccount,
};

use super::{
    assert_success, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_get, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal, OwnedMdbVal, MDB_NOTFOUND, MDB_SUCCESS,
};

pub struct LmdbUncheckedStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbUncheckedStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl UncheckedStore for LmdbUncheckedStore {
    fn clear(&self, txn: &dyn WriteTransaction) {
        let status =
            unsafe { mdb_drop(get_raw_lmdb_txn(txn.as_transaction()), self.table_handle, 0) };
        assert_success(status);
    }

    fn put(&self, txn: &dyn WriteTransaction, dependency: &HashOrAccount, info: &UncheckedInfo) {
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
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from_slice(&key_bytes),
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn exists(&self, txn: &dyn Transaction, key: &UncheckedKey) -> bool {
        let mut value = MdbVal::new();
        let key_bytes = key.to_bytes();
        let status = unsafe {
            mdb_get(
                get_raw_lmdb_txn(txn),
                self.table_handle,
                &mut MdbVal::from_slice(&key_bytes),
                &mut value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        status == MDB_SUCCESS
    }

    fn del(&self, txn: &dyn WriteTransaction, key: &UncheckedKey) {
        let key_bytes = key.to_bytes();
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from_slice(&key_bytes),
                None,
            )
        };
        assert_success(status);
    }

    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<UncheckedKey, UncheckedInfo>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, true))
    }

    fn lower_bound(
        &self,
        txn: &dyn Transaction,
        key: &UncheckedKey,
    ) -> Box<dyn DbIterator<UncheckedKey, UncheckedInfo>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, Some(key), true))
    }

    fn count(&self, txn: &dyn Transaction) -> usize {
        unsafe { mdb_count(get_raw_lmdb_txn(txn), self.table_handle) }
    }
}
