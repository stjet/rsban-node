pub struct BulkPullServer {
    pub sent_count: u32,
    pub max_count: u32,
}

impl BulkPullServer {
    pub fn new() -> Self {
        Self {
            sent_count: 0,
            max_count: 0,
        }
    }
}
