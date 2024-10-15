mod common;
//mod ledger;
//mod node;
//mod utils;
//mod wallets;

pub use common::*;
//pub use ledger::*;
//pub use node::*;
//pub use utils::*;
//pub use wallets::*;

use serde::{Deserialize, Serialize};

use crate::{AccountBlockCountDto, AccountRepresentativeDto, WalletRpcMessage};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcDto {
    AccountBalance(AccountBalanceDto),
    Account(AccountRpcMessage),
    Accounts(AccountsRpcMessage),
    Removed(RemovedDto),
    Moved(MovedDto),
    WalletCreate(WalletRpcMessage),
    KeyPair(KeyPairDto),
    Exists(ExistsDto),
    Error(ErrorDto2),
    Destroyed(DestroyedDto),
    Locked(LockedDto),
    Lock(LockedDto),
    Stop(SuccessDto),
    AccountBlockCount(AccountBlockCountDto),
    AccountKey(KeyRpcMessage),
    AccountGet(AccountRpcMessage),
    AccountRepresentative(AccountRepresentativeDto),
    AccountWeight(WeightDto)
}
