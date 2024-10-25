use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn populate_backlog(&self) -> SuccessDto {
        self.node.backlog_population.trigger();
        SuccessDto::new()
    }
}
