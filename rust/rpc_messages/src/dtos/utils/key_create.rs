use rsnano_core::{Account, PublicKey, RawKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyCreateDto {
    pub private: RawKey,
    pub public: PublicKey,
    pub account: Account,
}
