use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, BlockHash};
use rsnano_rpc_messages::{FrontiersArgs, FrontiersResponse};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn frontiers(&self, args: FrontiersArgs) -> FrontiersResponse {
        let tx = self.node.ledger.read_txn();
        let mut frontiers: HashMap<Account, BlockHash> = HashMap::new();
        let mut iterator = self.node.store.account.begin_account(&tx, &args.account);
        let mut collected = 0;

        while collected < args.count {
            if let Some((account, account_info)) = iterator.current() {
                frontiers.insert(*account, account_info.head);
                collected += 1;
                iterator.next();
            } else {
                break;
            }
        }

        FrontiersResponse::new(frontiers)
    }
}
