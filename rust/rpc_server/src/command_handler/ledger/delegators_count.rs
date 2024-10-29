use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountArg, CountRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn delegators_count(&self, args: AccountArg) -> CountRpcMessage {
        let representative = args.account;
        let mut count = 0;

        let tx = self.node.ledger.read_txn();
        let mut iter = self.node.store.account.begin(&tx);

        while let Some((_, info)) = iter.current() {
            if info.representative == representative.into() {
                count += 1;
            }

            iter.next();
        }
        CountRpcMessage::new(count)
    }
}