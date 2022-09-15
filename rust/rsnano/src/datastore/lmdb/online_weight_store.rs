use std::sync::Arc;

use crate::datastore::OnlineWeightStore;

use super::LmdbEnv;

pub struct LmdbOnlineWeightStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbOnlineWeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl OnlineWeightStore for LmdbOnlineWeightStore {}
