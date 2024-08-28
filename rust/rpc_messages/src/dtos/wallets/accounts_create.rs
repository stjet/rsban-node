use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsCreatedDto {
    pub accounts: Vec<Account>,
}

impl AccountsCreatedDto {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }
}
