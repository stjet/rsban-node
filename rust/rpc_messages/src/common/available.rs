use rsnano_core::Amount;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AvailableDto {
    pub available: Amount,
}

impl AvailableDto {
    pub fn new(available: Amount) -> Self {
        Self { available }
    }
}