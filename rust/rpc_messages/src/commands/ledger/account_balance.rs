use rsnano_core::{Account, Amount};
use serde::{
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceArgs {
    pub account: Account,
    pub include_only_confirmed: Option<bool>,
}
