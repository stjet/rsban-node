use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsCreateArgs {
    pub wallet: WalletId,
    pub count: u64,
}
