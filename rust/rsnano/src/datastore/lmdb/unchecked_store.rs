use std::sync::Arc;

use rand::{thread_rng, Rng};

use crate::{
    datastore::{
        parallel_traversal, DbIterator, NullIterator, PrunedStore, ReadTransaction, Transaction,
        UncheckedStore, WriteTransaction,
    },
    unchecked_info::{UncheckedInfo, UncheckedKey},
    BlockHash, HashOrAccount, NoValue,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal, OwnedMdbVal,
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
}
