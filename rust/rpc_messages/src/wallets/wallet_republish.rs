use rsnano_core::WalletId;
use crate::RpcCommand;
use super::WalletWithCountArgs;

impl RpcCommand {
    pub fn wallet_republish(wallet: WalletId, count: u64) -> Self {
        Self::WalletRepublish(WalletWithCountArgs::new(wallet, count))
    }
}