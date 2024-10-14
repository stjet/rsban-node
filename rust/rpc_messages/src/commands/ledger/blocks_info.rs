use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn blocks_info(hashes: Vec<BlockHash>) -> Self {
        Self::BlocksInfo(BlocksInfoArgs::new(hashes))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlocksInfoArgs {
    pub hashes: Vec<BlockHash>,
}

impl BlocksInfoArgs {
    pub fn new(hashes: Vec<BlockHash>) -> Self {
        Self { hashes }
    }
}
