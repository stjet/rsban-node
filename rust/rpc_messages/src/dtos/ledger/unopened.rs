use std::collections::HashMap;
use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnopenedDto {
    pub accounts: HashMap<Account, Amount>,
}

impl UnopenedDto {
    pub fn new(accounts: HashMap<Account, Amount>) -> Self {
        Self { accounts }
    }
}