use crate::command_handler::RpcCommandHandler;
use rsnano_core::Amount;
use rsnano_rpc_messages::{AccountBalanceResponse, AccountsBalancesArgs, AccountsBalancesDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn accounts_balances(&self, args: AccountsBalancesArgs) -> AccountsBalancesDto {
        let tx = self.node.ledger.read_txn();
        let mut balances = HashMap::new();
        let only_confirmed = args.include_only_confirmed.unwrap_or(true);

        for account in args.accounts {
            let balance = if only_confirmed {
                self.node
                    .ledger
                    .confirmed()
                    .account_balance(&tx, &account)
                    .unwrap_or(Amount::zero())
            } else {
                self.node
                    .ledger
                    .any()
                    .account_balance(&tx, &account)
                    .unwrap_or(Amount::zero())
            };

            let pending = self
                .node
                .ledger
                .account_receivable(&tx, &account, only_confirmed);

            balances.insert(
                account,
                AccountBalanceResponse {
                    balance,
                    pending,
                    receivable: pending,
                },
            );
        }

        AccountsBalancesDto { balances }
    }
}
