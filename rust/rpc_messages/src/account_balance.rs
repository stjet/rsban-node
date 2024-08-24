use rsnano_core::{Account, Amount};
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceRequest {
    pub account: Account,
    pub include_only_confirmed: Option<bool>,
}

#[derive(PartialEq, Eq, Debug, Deserialize)]
pub struct AccountBalanceResponse {
    pub balance: Amount,
    pub pending: Amount,
    pub receivable: Amount,
}

impl AccountBalanceResponse {
    pub fn new(balance: Amount, pending: Amount, receivable: Amount) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}

impl Serialize for AccountBalanceResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AccountBalance", 3)?;
        state.serialize_field("balance", &self.balance.to_string_dec())?;
        state.serialize_field("pending", &self.pending.to_string_dec())?;
        state.serialize_field("receivable", &self.receivable.to_string_dec())?;
        state.end()
    }
}
