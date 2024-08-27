use rsnano_core::Amount;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
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
