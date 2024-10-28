use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::AmountRpcMessage;

impl RpcCommandHandler {
    pub(crate) fn receive_minimum(&self) -> AmountRpcMessage {
        let amount = self.node.config.receive_minimum;
        AmountRpcMessage::new(amount)
    }
}
