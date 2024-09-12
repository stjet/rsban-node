use crate::{LedgerArgs, RpcCommand};
use rsnano_core::{Account, Amount, WalletId};

impl RpcCommand {
    pub fn ledger(
        wallet: WalletId,
        representative: Option<Account>,
        weight: Option<Amount>,
        receivable: Option<bool>,
        modified_since: Option<u64>,
        sorting: Option<bool>,
        threshold: Option<Amount>
    ) -> Self {
        Self::Ledger(LedgerArgs::new(wallet, representative, weight, receivable, modified_since, sorting, threshold))
    }
}