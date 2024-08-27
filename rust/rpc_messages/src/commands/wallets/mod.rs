mod account_create;
mod receive;
mod send;
mod wallet_add;

use super::RpcCommand;
pub use account_create::*;
pub use receive::*;
use rsnano_core::{RawKey, WalletId};
pub use send::*;
pub use wallet_add::*;

impl RpcCommand {
    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddArgs {
            wallet: wallet_id,
            key,
        })
    }

    pub fn account_create(wallet: WalletId, index: Option<u32>) -> Self {
        Self::AccountCreate(AccountCreateArgs { wallet, index })
    }
}
