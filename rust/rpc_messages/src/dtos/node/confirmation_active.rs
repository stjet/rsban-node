use rsnano_core::QualifiedRoot;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationActiveDto {
    pub confirmations: Vec<QualifiedRoot>,
    pub unconfirmed: u64,
    pub confirmed: u64,
}

impl ConfirmationActiveDto {
    pub fn new(confirmations: Vec<QualifiedRoot>, unconfirmed: u64, confirmed: u64) -> Self {
        Self {
            confirmations,
            unconfirmed,
            confirmed,
        }
    }
}
