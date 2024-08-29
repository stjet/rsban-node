use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountListDto {
    pub accounts: Vec<Account>,
}

impl AccountListDto {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }
}
