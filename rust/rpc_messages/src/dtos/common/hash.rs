use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HashDto {
    pub hash: BlockHash,
}

impl HashDto {
    pub fn new(hash: BlockHash) -> Self {
        Self { hash }
    }
}
