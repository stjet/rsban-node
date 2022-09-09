use std::sync::Arc;

use super::LmdbEnv;

pub struct LmdbConfirmationHeightStore {
    env: Arc<LmdbEnv>,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self { env }
    }
}
