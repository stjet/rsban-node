use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountRepresentativeDto {
    pub representative: Account,
}

impl AccountRepresentativeDto {
    pub fn new(representative: Account) -> Self {
        Self { representative }
    }
}
