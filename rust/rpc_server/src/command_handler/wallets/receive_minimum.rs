use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AmountRpcMessage, ErrorDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn receive_minimum(&self) -> RpcDto {
        if self.enable_control {
            let amount = self.node.config.receive_minimum;
            RpcDto::ReceiveMinimum(AmountRpcMessage::new(amount))
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
