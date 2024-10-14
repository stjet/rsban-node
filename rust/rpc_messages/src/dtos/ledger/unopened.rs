use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnopenedDto {
    pub accounts: HashMap<Account, Amount>,
}

impl UnopenedDto {
    pub fn new(accounts: HashMap<Account, Amount>) -> Self {
        Self { accounts }
    }
}
