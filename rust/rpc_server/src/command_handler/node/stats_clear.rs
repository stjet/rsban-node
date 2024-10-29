use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::SuccessResponse;

impl RpcCommandHandler {
    pub(crate) fn stats_clear(&self) -> SuccessResponse {
        self.node.stats.clear();
        SuccessResponse::new()
    }
}
