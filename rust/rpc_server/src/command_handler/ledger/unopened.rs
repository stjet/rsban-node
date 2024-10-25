use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, Amount};
use rsnano_rpc_messages::{UnopenedArgs, UnopenedDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn unopened(&self, args: UnopenedArgs) -> anyhow::Result<UnopenedDto> {
        self.ensure_control_enabled()?;

        let start = args.account;
        let mut accounts: HashMap<Account, Amount> = HashMap::new();

        let transaction = self.node.store.tx_begin_read();
        let mut iterator = self.node.store.pending.begin(&transaction);
        let end = self.node.store.pending.end();

        let mut current_account = start;
        let mut current_account_sum = Amount::zero();

        while iterator != end && accounts.len() < args.count as usize {
            let (key, info) = iterator.current().unwrap();
            let account = key.receiving_account;

            if self
                .node
                .store
                .account
                .get(&transaction, &account)
                .is_some()
            {
                iterator = self.node.store.pending.begin_at_key(&transaction, key);
            } else {
                if account != current_account {
                    if !current_account_sum.is_zero() {
                        if args.threshold.map_or(true, |t| current_account_sum >= t) {
                            accounts.insert(current_account, current_account_sum);
                        }
                        current_account_sum = Amount::zero();
                    }
                    current_account = account;
                }
                current_account_sum += info.amount;
            }
            iterator.next();
        }

        if accounts.len() < args.count as usize
            && !current_account_sum.is_zero()
            && args.threshold.map_or(true, |t| current_account_sum >= t)
        {
            accounts.insert(current_account, current_account_sum);
        }

        Ok(UnopenedDto::new(accounts))
    }
}
