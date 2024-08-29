use rsnano_core::{Account, PublicKey, RawKey};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct KeyPairDto {
    pub private: RawKey,
    pub public: PublicKey,
    pub account: Account,
}

impl KeyPairDto {
    pub fn new(private: RawKey, public: PublicKey, account: Account) -> Self {
        Self {
            private,
            public,
            account,
        }
    }
}
