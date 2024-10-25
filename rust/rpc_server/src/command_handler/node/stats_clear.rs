use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn stats_clear(&self) -> SuccessDto {
        self.node.stats.clear();
        SuccessDto::new()
    }
}
