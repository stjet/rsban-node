use std::sync::Arc;

use crate::{
    datastore::{DbIterator, PeerStore, Transaction, WriteTransaction},
    Amount,
};

use super::{
    assert_success, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_put, LmdbEnv, LmdbIterator,
    MdbVal,
};

pub struct LmdbPeerStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbPeerStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}
