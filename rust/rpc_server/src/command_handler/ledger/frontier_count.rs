use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::CountRpcMessage;

impl RpcCommandHandler {
    pub(crate) fn frontier_count(&self) -> CountRpcMessage {
        CountRpcMessage::new(self.node.ledger.account_count())
    }
}
