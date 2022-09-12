use std::sync::Arc;

use crate::{
    datastore::{
        lmdb::{assert_success, mdb_put, MDB_NOTFOUND, MDB_SUCCESS},
        FinalVoteStore, WriteTransaction,
    },
    BlockHash, QualifiedRoot,
};

use super::{get_raw_lmdb_txn, mdb_get, LmdbEnv, MdbVal};

/// Maps root to block hash for generated final votes.
/// nano::qualified_root -> nano::block_hash
pub struct LmdbFinalVoteStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbFinalVoteStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl FinalVoteStore for LmdbFinalVoteStore {
    fn put(&self, txn: &dyn WriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool {
        let mut value = MdbVal::new();
        let root_bytes = root.to_bytes();
        let status = unsafe {
            mdb_get(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut MdbVal::from_slice(&root_bytes),
                &mut value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        if status == MDB_SUCCESS {
            BlockHash::try_from(&value).unwrap() == *hash
        } else {
            let status = unsafe {
                mdb_put(
                    get_raw_lmdb_txn(txn.as_transaction()),
                    self.table_handle,
                    &mut MdbVal::from_slice(&root_bytes),
                    &mut MdbVal::from(hash),
                    0,
                )
            };
            assert_success(status);
            true
        }
    }
}
