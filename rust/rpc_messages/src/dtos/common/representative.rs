use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RepresentativeDto {
    pub representative: Account,
}

impl RepresentativeDto {
    pub fn new(representative: Account) -> Self {
        Self { representative }
    }
}
