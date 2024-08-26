mod account_remove;
mod receive;
mod send;
mod wallet_add;

pub use account_remove::*;
pub use receive::*;
use rsnano_core::{Account, RawKey, WalletId};
pub use send::*;
use serde::{Deserialize, Serialize};
pub use wallet_add::*;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WalletsRpcCommand {
    Receive(ReceiveArgs),
    Send(SendArgs),
    WalletAdd(WalletAddArgs),
    WalletCreate,
    AccountRemove(AccountRemoveArgs),
}

impl WalletsRpcCommand {
    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddArgs {
            wallet: wallet_id,
            key,
        })
    }

    pub fn account_remove(wallet: WalletId, account: Account) -> Self {
        Self::AccountRemove(AccountRemoveArgs { wallet, account })
    }
}
