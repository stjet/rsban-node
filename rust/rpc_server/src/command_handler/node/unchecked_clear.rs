use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn unchecked_clear(&self) -> RpcDto {
        self.node.unchecked.clear();
        RpcDto::UncheckedClear(SuccessDto::new())
    }
}
