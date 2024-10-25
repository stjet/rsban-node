use crate::command_handler::RpcCommandHandler;
use rsnano_core::Amount;
use rsnano_rpc_messages::{AccountBalanceDto, AccountsBalancesDto, WalletBalancesArgs};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn wallet_balances(&self, args: WalletBalancesArgs) -> AccountsBalancesDto {
        let threshold = args.threshold.unwrap_or(Amount::zero());
        let accounts = self
            .node
            .wallets
            .get_accounts_of_wallet(&args.wallet)
            .unwrap();
        let mut balances = HashMap::new();
        let tx = self.node.ledger.read_txn();
        for account in accounts {
            let balance = match self.node.ledger.any().account_balance(&tx, &account) {
                Some(balance) => balance,
                None => Amount::zero(),
            };

            let pending = self.node.ledger.account_receivable(&tx, &account, false);

            let account_balance = AccountBalanceDto::new(balance, pending, pending);
            if balance >= threshold {
                balances.insert(account, account_balance);
            }
        }
        AccountsBalancesDto { balances }
    }
}
