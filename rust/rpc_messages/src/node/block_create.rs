use rsnano_core::{Account, Amount, BlockHash, BlockSubType, Link, RawKey, WalletId, WorkNonce, WorkVersion};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn block_create(
        block_type: BlockSubType,
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
    block_type: BlockSubType,
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
}

impl BlockCreateArgs {
    pub fn new(
        block_type: BlockSubType,
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