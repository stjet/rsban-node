use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, Amount};
use rsnano_rpc_messages::{DelegatorsArgs, DelegatorsDto, RpcDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn delegators(&self, args: DelegatorsArgs) -> RpcDto {
        let representative = args.account;
        let count = args.count.unwrap_or(1024);
        let threshold = args.threshold.unwrap_or(Amount::zero());
        let start_account = args.start.unwrap_or(Account::zero());

        let mut delegators: HashMap<Account, Amount> = HashMap::new();
        let tx = self.node.ledger.read_txn();
        let mut iter = self.node.store.account.begin_account(&tx, &start_account);

        while let Some((account, info)) = iter.current() {
            if delegators.len() >= count as usize {
                break;
            }

            if info.representative == representative.into() && info.balance >= threshold {
                delegators.insert(*account, info.balance);
            }

            iter.next();
        }
        RpcDto::Delegators(DelegatorsDto::new(delegators))
    }
}
