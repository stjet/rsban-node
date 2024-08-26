use rsnano_core::{Account, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountRemoveArgs {
    pub wallet: WalletId,
    pub account: Account,
}
