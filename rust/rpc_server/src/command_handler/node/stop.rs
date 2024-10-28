use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn stop(&self) -> SuccessDto {
        self.node.stop();
        SuccessDto::new()
    }
}
