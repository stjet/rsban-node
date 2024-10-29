use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::SuccessResponse;

impl RpcCommandHandler {
    pub(crate) fn stop(&self) -> SuccessResponse {
        self.node.stop();
        SuccessResponse::new()
    }
}
