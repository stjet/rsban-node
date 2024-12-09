use crate::command_handler::RpcCommandHandler;
use rsban_rpc_messages::SuccessResponse;

impl RpcCommandHandler {
    pub(crate) fn unchecked_clear(&self) -> SuccessResponse {
        self.node.unchecked.clear();
        SuccessResponse::new()
    }
}
