use rsnano_core::{Account, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountMoveArgs {
    pub wallet: WalletId,
    pub source: WalletId,
    pub accounts: Vec<Account>,
}
