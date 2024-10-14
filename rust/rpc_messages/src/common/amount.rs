use rsnano_core::Amount;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AmountDto {
    pub amount: Amount,
}

impl AmountDto {
    pub fn new(amount: Amount) -> Self {
        Self { amount }
    }
}