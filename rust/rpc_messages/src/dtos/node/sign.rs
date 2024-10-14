use rsnano_core::{Account, JsonBlock, RawKey, Signature, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SignDto {
    pub signature: Signature,
    pub block: JsonBlock,
}

impl SignDto {
    pub fn new(signature: Signature, block: JsonBlock) -> Self {
        Self { signature, block }
    }
}
