use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsCreateDto {
    pub accounts: Vec<Account>,
}

impl AccountsCreateDto {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }
}
