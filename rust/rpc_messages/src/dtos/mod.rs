mod common;
mod ledger;
mod node;
mod utils;
mod wallets;

pub use common::*;
pub use ledger::*;
pub use node::*;
pub use utils::*;
pub use wallets::*;

use serde::{Deserialize, Serialize};

use crate::AccountBlockCountDto;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcDto {
    AccountBalance(AccountBalanceDto),
    Account(AccountDto),
    Accounts(AccountsDto),
    Removed(RemovedDto),
    Moved(MovedDto),
    WalletCreate(WalletCreateDto),
    KeyPair(KeyPairDto),
    Exists(ExistsDto),
    Error(ErrorDto2),
    Destroyed(DestroyedDto),
    Locked(LockedDto),
    Lock(LockDto),
    Stop(SuccessDto),
    AccountBlockCount(AccountBlockCountDto),
    AccountKey(KeyDto),
    AccountGet(AccountDto),
    AccountRepresentative(AccountRepresentativeDto),
    AccountWeight(WeightDto)
}
