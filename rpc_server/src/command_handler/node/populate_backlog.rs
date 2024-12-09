use crate::command_handler::RpcCommandHandler;
use rsban_rpc_messages::SuccessResponse;

impl RpcCommandHandler {
    pub(crate) fn populate_backlog(&self) -> SuccessResponse {
        self.node.backlog_population.trigger();
        SuccessResponse::new()
    }
}
