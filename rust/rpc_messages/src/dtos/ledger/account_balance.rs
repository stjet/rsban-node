use rsnano_core::Amount;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

#[derive(PartialEq, Eq, Debug, Deserialize)]
pub struct AccountBalanceDto {
    pub balance: Amount,
    pub pending: Amount,
    pub receivable: Amount,
}

impl AccountBalanceDto {
    pub fn new(balance: Amount, pending: Amount, receivable: Amount) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}

impl Serialize for AccountBalanceDto {
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
