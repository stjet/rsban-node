mod account_remove;
mod receive_args;
mod send_args;
mod wallet_add_args;

use super::RpcCommand;
pub use account_remove::*;
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

    pub fn account_remove(wallet: WalletId, account: Account) -> Self {
        Self::AccountRemove(AccountRemoveArgs { wallet, account })
    }
}
