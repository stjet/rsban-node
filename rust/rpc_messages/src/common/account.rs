use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountDto {
    pub account: Account,
}

impl AccountDto {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

