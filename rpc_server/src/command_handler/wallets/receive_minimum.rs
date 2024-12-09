use crate::command_handler::RpcCommandHandler;
use rsban_rpc_messages::AmountRpcMessage;

impl RpcCommandHandler {
    pub(crate) fn receive_minimum(&self) -> AmountRpcMessage {
        AmountRpcMessage::new(self.node.config.receive_minimum)
    }
}
