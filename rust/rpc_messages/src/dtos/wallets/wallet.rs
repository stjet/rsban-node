use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletDto {
    pub wallet: WalletId,
}

impl WalletDto {
    pub fn new(wallet: WalletId) -> Self {
        Self { wallet }
    }
}
