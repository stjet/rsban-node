use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn stats_clear(&self) -> RpcDto {
        self.node.stats.clear();
        RpcDto::StatsClear(SuccessDto::new())
    }
}
