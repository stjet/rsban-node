use rsnano_core::{Account, PublicKey, RawKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyPairResponse {
    pub private_key: RawKey,
    pub public_key: PublicKey,
    pub as_string: Account,
}
