use crate::command_handler::RpcCommandHandler;
use rsnano_core::Amount;
use rsnano_rpc_messages::{AccountBalanceArgs, AccountBalanceDto};

impl RpcCommandHandler {
    pub(crate) fn account_balance(&self, args: AccountBalanceArgs) -> AccountBalanceDto {
        let tx = self.node.ledger.read_txn();
        let include_unconfirmed_blocks = args.include_only_confirmed.unwrap_or(false);

        let balance = if !include_unconfirmed_blocks {
            self.node
                .ledger
                .confirmed()
                .account_balance(&tx, &args.account)
                .unwrap_or(Amount::zero())
        } else {
            self.node
                .ledger
                .any()
                .account_balance(&tx, &args.account)
                .unwrap_or(Amount::zero())
        };

        let pending =
            self.node
                .ledger
                .account_receivable(&tx, &args.account, !include_unconfirmed_blocks);

        AccountBalanceDto::new(balance, pending, pending)
    }
}
