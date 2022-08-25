use crate::BlockHash;
use std::sync::Arc;

use super::{assert_success, mdb_put, LmdbEnv, LmdbWriteTransaction, MdbVal, OwnedMdbVal};

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
}
