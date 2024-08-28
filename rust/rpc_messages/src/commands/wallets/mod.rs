mod accounts_create;
mod receive_args;
mod send_args;
mod wallet_add_args;

use super::RpcCommand;
pub use accounts_create::*;
pub use receive_args::*;
use rsnano_core::{RawKey, WalletId};
pub use send_args::*;
pub use wallet_add_args::*;

impl RpcCommand {
    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddArgs {
            wallet: wallet_id,
            key,
        })
    }

    pub fn accounts_create(wallet: WalletId, count: u64) -> Self {
        Self::AccountsCreate(AccountsCreateArgs { wallet, count })
    }
}
