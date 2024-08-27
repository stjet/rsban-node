use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountCreatedDto {
    pub account: Account,
}

impl AccountCreatedDto {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}
