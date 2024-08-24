use rsnano_core::{Account, Amount, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub wallet: WalletId,
    pub source: Account,
    pub destination: Account,
    pub amount: Amount,
}
