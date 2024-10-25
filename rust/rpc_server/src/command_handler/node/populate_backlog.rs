use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn populate_backlog(&self) -> RpcDto {
        self.node.backlog_population.trigger();
        RpcDto::PopulateBacklog(SuccessDto::new())
    }
}
