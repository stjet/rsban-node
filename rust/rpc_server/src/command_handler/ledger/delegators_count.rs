use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountRpcMessage, CountRpcMessage, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn delegators_count(&self, args: AccountRpcMessage) -> RpcDto {
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
        RpcDto::DelegatorsCount(CountRpcMessage::new(count))
    }
}
