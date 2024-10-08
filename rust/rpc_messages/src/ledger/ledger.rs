use crate::{LedgerArgs, RpcCommand};
use rsnano_core::{Account, Amount};

impl RpcCommand {
    pub fn ledger(
        account: Option<Account>,
        count: Option<u64>,
        representative: Option<bool>,
        weight: Option<bool>,
        receivable: Option<bool>,
        modified_since: Option<u64>,
        sorting: Option<bool>,
        threshold: Option<Amount>
    ) -> Self {
        Self::Ledger(LedgerArgs::new(account, count, representative, weight, receivable, modified_since, sorting, threshold))
    }
}