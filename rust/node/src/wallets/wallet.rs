use std::{collections::HashSet, sync::Mutex};

use rsnano_core::Account;

pub struct Wallet {
    pub representatives: Mutex<HashSet<Account>>,
}

impl Wallet {
    pub fn new() -> Self {
        Self {
            representatives: Mutex::new(HashSet::new()),
        }
    }
}
