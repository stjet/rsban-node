use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn accounts_receivable(
        accounts: Vec<Account>,
        count: u64,
        threshold: Option<Amount>,
        source: Option<bool>,
        include_active: Option<bool>,
        sorting: Option<bool>,
        include_only_confirmed: Option<bool>
    ) -> Self {
        Self::AccountsReceivable(AccountsReceivableArgs {
            accounts,
            count,
            threshold,
            source,
            include_active,
            sorting,
            include_only_confirmed
        })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsReceivableArgs {
    pub accounts: Vec<Account>,
    pub count: u64,
    pub threshold: Option<Amount>,
    pub source: Option<bool>,
    pub include_active: Option<bool>,
    pub sorting: Option<bool>,
    pub include_only_confirmed: Option<bool>
}