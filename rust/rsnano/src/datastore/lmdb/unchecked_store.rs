use std::sync::Arc;

use rand::{thread_rng, Rng};

use crate::{
    datastore::{
        parallel_traversal, DbIterator, NullIterator, PrunedStore, ReadTransaction, Transaction,
        WriteTransaction,
    },
    BlockHash, NoValue,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal,
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
