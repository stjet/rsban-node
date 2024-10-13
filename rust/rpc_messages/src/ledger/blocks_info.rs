use std::collections::HashMap;
use crate::{BlockInfoDto, RpcCommand};
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn blocks_info(blocks: Vec<BlockHash>) -> Self {
        Self::BlocksInfo(BlocksInfoArgs::new(blocks))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlocksInfoArgs {
    pub hashes: Vec<BlockHash>
}

impl BlocksInfoArgs {
    pub fn new(hashes: Vec<BlockHash>) -> Self {
        Self { hashes }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlocksInfoDto {
    blocks: HashMap<BlockHash, BlockInfoDto>,
}

impl BlocksInfoDto {
    pub fn new(blocks: HashMap<BlockHash, BlockInfoDto>) -> Self {
        Self { blocks }
    }
}
