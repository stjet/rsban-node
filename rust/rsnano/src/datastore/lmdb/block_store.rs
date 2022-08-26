use crate::{
    datastore::{
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        Transaction,
    },
    BlockHash,
};
use std::sync::Arc;

use super::{
    assert_success, get_raw_lmdb_txn, mdb_get, mdb_put, LmdbEnv, LmdbWriteTransaction, MdbVal,
    OwnedMdbVal,
};

pub struct LmdbBlockStore {
    env: Arc<LmdbEnv>,
    pub blocks_handle: u32,
}

impl LmdbBlockStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            blocks_handle: 0,
        }
    }

    pub fn raw_put(&self, txn: &mut LmdbWriteTransaction, data: &[u8], hash: BlockHash) {
        let mut key = OwnedMdbVal::from(&hash);
        let mut data = MdbVal::from_slice(data);
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.blocks_handle,
                key.as_mdb_val(),
                &mut data,
                0,
            )
        };
        assert_success(status);
    }

    pub fn block_raw_get(&self, txn: &dyn Transaction, hash: BlockHash, value: &mut MdbVal) {
        let mut key = OwnedMdbVal::from(&hash);
        let status = unsafe {
            mdb_get(
                get_raw_lmdb_txn(txn),
                self.blocks_handle,
                key.as_mdb_val(),
                value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
    }
}
