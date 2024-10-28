use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn search_receivable_all(&self) -> SuccessDto {
        self.node.search_receivable_all();
        SuccessDto::new()
    }
}
