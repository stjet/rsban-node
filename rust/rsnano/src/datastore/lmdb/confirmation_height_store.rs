use std::sync::Arc;

use super::LmdbEnv;

pub struct LmdbConfirmationHeightStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}
