use rsnano_core::{Account, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountCreateArgs {
    pub wallet: WalletId,
    pub index: Option<u32>,
}
