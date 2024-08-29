use super::AccountBalanceDto;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletBalancesDto {
    balances: HashMap<Account, AccountBalanceDto>,
}

impl WalletBalancesDto {
    pub fn new(balances: HashMap<Account, AccountBalanceDto>) -> Self {
        Self { balances }
    }
}
