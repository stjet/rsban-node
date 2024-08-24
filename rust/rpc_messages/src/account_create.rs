use rsnano_core::{Account, WalletId};
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountCreateRequest {
    pub wallet: WalletId,
    pub index: Option<u32>,
}

#[derive(PartialEq, Eq, Debug, Deserialize)]
pub struct AccountCreateResponse {
    pub account: Account,
}

impl AccountCreateResponse {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

impl Serialize for AccountCreateResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AccountCreateResponse", 1)?;
        state.serialize_field("account", &self.account.to_string())?;
        state.end()
    }
}
