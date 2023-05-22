use rsnano_core::BlockHash;

use crate::messages::BulkPull;

pub struct BulkPullServer {
    pub sent_count: u32,
    pub max_count: u32,
    pub include_start: bool,
    pub current: BlockHash,
    pub request: BulkPull,
}

impl BulkPullServer {
    pub fn new(request: BulkPull) -> Self {
        Self {
            sent_count: 0,
            max_count: 0,
            include_start: false,
            current: BlockHash::zero(),
            request,
        }
    }
}
