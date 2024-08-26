use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountCreateDto {
    pub account: Account,
}

impl AccountCreateDto {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}
