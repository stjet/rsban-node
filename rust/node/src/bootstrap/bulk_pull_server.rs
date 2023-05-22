use rsnano_core::BlockHash;

pub struct BulkPullServer {
    pub sent_count: u32,
    pub max_count: u32,
    pub include_start: bool,
    pub current: BlockHash,
}

impl BulkPullServer {
    pub fn new() -> Self {
        Self {
            sent_count: 0,
            max_count: 0,
            include_start: false,
            current: BlockHash::zero(),
        }
    }
}
