use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn stop(&self) -> RpcDto {
        if self.enable_control {
            self.node.stop();
            RpcDto::Stop(SuccessDto::new())
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
