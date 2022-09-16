use std::sync::Arc;

use crate::{
    datastore::{DbIterator, PendingStore, Transaction, WriteTransaction},
    EndpointKey, NoValue,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv,
    LmdbIterator, MdbVal, OwnedMdbVal,
};

pub struct LmdbPendingStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbPendingStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}
