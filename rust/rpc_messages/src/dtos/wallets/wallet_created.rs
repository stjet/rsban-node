use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletCreatedDto {
    pub wallet: WalletId,
}

impl WalletCreatedDto {
    pub fn new(wallet: WalletId) -> Self {
        Self { wallet }
    }
}
