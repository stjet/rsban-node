use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn blocks(blocks: Vec<BlockHash>) -> Self {
        Self::Blocks(BlocksArgs::new(blocks))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlocksArgs {
    pub hashes: Vec<BlockHash>,
}

impl BlocksArgs {
    pub fn new(hashes: Vec<BlockHash>) -> Self {
        Self { hashes }
    }
}
