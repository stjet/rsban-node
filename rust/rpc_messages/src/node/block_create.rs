use rsnano_core::{Account, Amount, BlockHash, BlockType, JsonBlock, Link, RawKey, WalletId, WorkNonce, WorkVersion};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn block_create(
        block_type: BlockType,
        balance: Option<Amount>,
        key: Option<RawKey>,
        wallet: Option<WalletId>,
        account: Option<Account>,
        source: Option<BlockHash>,
        destination: Option<Account>,
        representative: Option<Account>,
        link: Option<Link>,
        previous: Option<BlockHash>,
        work: Option<WorkNonce>,
        version: Option<WorkVersion>,
        difficulty: Option<u64>,
    ) -> Self {
        Self::BlockCreate(BlockCreateArgs::new(
            block_type,
            balance,
            key,
            wallet,
            account,
            source,
            destination,
            representative,
            link,
            previous,
            work,
            version,
            difficulty,
        ))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCreateArgs {
    #[serde(rename = "type")]
    pub block_type: BlockType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<RawKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<WalletId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<BlockHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<BlockHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work: Option<WorkNonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<WorkVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<u64>,
}

impl BlockCreateArgs {
    pub fn new(
        block_type: BlockType,
        balance: Option<Amount>,
        key: Option<RawKey>,
        wallet: Option<WalletId>,
        account: Option<Account>,
        source: Option<BlockHash>,
        destination: Option<Account>,
        representative: Option<Account>,
        link: Option<Link>,
        previous: Option<BlockHash>,
        work: Option<WorkNonce>,
        version: Option<WorkVersion>,
        difficulty: Option<u64>,
    ) -> Self {
        Self {
            block_type,
            balance,
            key,
            wallet,
            account,
            source,
            destination,
            representative,
            link,
            previous,
            work,
            version,
            difficulty,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCreateDto {
    pub hash: BlockHash,
    pub difficulty: u64,
    pub block: JsonBlock
}

impl BlockCreateDto {
    pub fn new(hash: BlockHash, difficulty: u64, block: JsonBlock) -> Self {
        Self {
            hash,
            difficulty,
            block,
        }
    }
}
