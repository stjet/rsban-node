use crate::RpcCommand;
use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn accounts_create(wallet: WalletId, count: u64) -> Self {
        Self::AccountsCreate(AccountsCreateArgs { wallet, count })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsCreateArgs {
    pub wallet: WalletId,
    pub count: u64,
}
