use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCountDto {
    pub block_count: u64,
}

impl BlockCountDto {
    pub fn new(block_count: u64) -> Self {
        Self { block_count }
    }
}