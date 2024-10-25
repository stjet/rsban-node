use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn unchecked_clear(&self) -> SuccessDto {
        self.node.unchecked.clear();
        SuccessDto::new()
    }
}
