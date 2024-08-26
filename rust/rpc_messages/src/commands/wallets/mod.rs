mod accounts_create;
mod receive;
mod send;
mod wallet_add;

pub use accounts_create::*;
pub use receive::*;
use rsnano_core::{RawKey, WalletId};
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
    AccountsCreate(AccountsCreateArgs),
}

impl WalletsRpcCommand {
    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddArgs {
            wallet: wallet_id,
            key,
        })
    }
}
