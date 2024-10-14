use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn block_info(block: BlockHash) -> Self {
        Self::BlockInfo(BlockInfoArgs::new(block))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockInfoArgs {
    pub block: BlockHash
}

impl BlockInfoArgs {
    pub fn new(block: BlockHash) -> Self {
        Self { block }
    }
}