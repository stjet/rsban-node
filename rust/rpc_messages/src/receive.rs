use rsnano_core::{Account, JsonBlock, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceiveRequest {
    pub wallet: WalletId,
    pub account: Account,
    pub block: JsonBlock,
}
