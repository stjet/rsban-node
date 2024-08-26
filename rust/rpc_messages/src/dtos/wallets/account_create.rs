use rsnano_core::PublicKey;
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

#[derive(PartialEq, Eq, Debug, Deserialize)]
pub struct AccountCreateDto {
    pub account: PublicKey,
}

impl AccountCreateDto {
    pub fn new(account: PublicKey) -> Self {
        Self { account }
    }
}

impl Serialize for AccountCreateDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AccountCreateResponse", 1)?;
        state.serialize_field("account", &self.account.encode_hex())?;
        state.end()
    }
}
