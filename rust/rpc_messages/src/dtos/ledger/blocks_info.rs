use super::BlockInfoDto;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlocksInfoDto {
    blocks: HashMap<BlockHash, BlockInfoDto>,
}

impl BlocksInfoDto {
    pub fn new(blocks: HashMap<BlockHash, BlockInfoDto>) -> Self {
        Self { blocks }
    }
}
