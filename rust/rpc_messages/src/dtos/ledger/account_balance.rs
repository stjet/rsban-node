use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceDto {
    pub balance: u128,
    pub pending: u128,
    pub receivable: u128,
}

impl AccountBalanceDto {
    pub fn new(balance: u128, pending: u128, receivable: u128) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}
