use std::collections::HashMap;
use rsnano_core::{Account, Amount, BlockHash, BlockSubType, JsonBlock};
use crate::{BlocksHashesRpcMessage, RpcCommand};
use serde::{Serialize, Deserialize};

impl RpcCommand {
    pub fn blocks_info(blocks: Vec<BlockHash>) -> Self {
        Self::BlocksInfo(BlocksHashesRpcMessage::new("hashes".to_string(), blocks))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlocksInfoDto {
    blocks: HashMap<BlockHash, BlockInfoDto>
}

impl BlocksInfoDto {
    pub fn new(blocks: HashMap<BlockHash, BlockInfoDto>) -> Self {
        Self { blocks }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockInfoDto {
    block_account: Account,
    amount: Amount,
    balance: Amount,
    height: u64,
    local_timestamp: u64,
    successor: BlockHash,
    confirmed: bool,
    contents: JsonBlock,
    subtype: BlockSubType,
}

impl BlockInfoDto {
    pub fn new(
        block_account: Account,
        amount: Amount,
        balance: Amount,
        height: u64,
        local_timestamp: u64,
        successor: BlockHash,
        confirmed: bool,
        contents: JsonBlock,
        subtype: BlockSubType,
    ) -> Self {
        Self {
            block_account,
            amount,
            balance,
            height,
            local_timestamp,
            successor,
            confirmed,
            contents,
            subtype,
        }
    }
}

