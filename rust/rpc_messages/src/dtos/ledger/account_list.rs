use rsnano_core::PublicKey;
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

#[derive(PartialEq, Eq, Debug, Deserialize)]
pub struct AccountListDto {
    pub accounts: Vec<PublicKey>,
}

impl AccountListDto {
    pub fn new(accounts: Vec<PublicKey>) -> Self {
        Self { accounts }
    }
}

impl Serialize for AccountListDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AccountListDto", 1)?;
        let account_strings: Vec<String> = self.accounts.iter().map(|pk| pk.encode_hex()).collect();

        state.serialize_field("accounts", &account_strings)?;
        state.end()
    }
}
