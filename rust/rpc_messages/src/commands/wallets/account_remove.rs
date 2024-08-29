use crate::RpcCommand;
use rsnano_core::{Account, WalletId};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn account_remove(wallet: WalletId, account: Account) -> Self {
        Self::AccountRemove(AccountRemoveArgs { wallet, account })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountRemoveArgs {
    pub wallet: WalletId,
    pub account: Account,
}
