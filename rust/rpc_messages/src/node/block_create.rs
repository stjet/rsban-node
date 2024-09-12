use rsnano_core::{Account, Amount, BlockHash, BlockType, JsonBlock, Link, RawKey, WalletId, WorkNonce, WorkVersion};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn block_create(
        block_type: BlockType,
        balance: Amount,
        key: Option<RawKey>,
        wallet: Option<WalletId>,
        account: Option<Account>,
        source: Option<BlockHash>,
        destination: Option<Account>,
        representative: Account,
        link: Option<Link>,
        previous: BlockHash,
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
    pub balance: Amount,
    pub key: Option<RawKey>,
    pub wallet: Option<WalletId>,
    pub account: Option<Account>,
    pub source: Option<BlockHash>,
    pub destination: Option<Account>,
    pub representative: Account,
    pub link: Option<Link>,
    pub previous: BlockHash,
    pub work: Option<WorkNonce>,
    pub version: Option<WorkVersion>,
    pub difficulty: Option<u64>,
}

impl BlockCreateArgs {
    pub fn new(
        block_type: BlockType,
        balance: Amount,
        key: Option<RawKey>,
        wallet: Option<WalletId>,
        account: Option<Account>,
        source: Option<BlockHash>,
        destination: Option<Account>,
        representative: Account,
        link: Option<Link>,
        previous: BlockHash,
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
