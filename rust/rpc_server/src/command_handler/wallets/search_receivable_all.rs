use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn search_receivable_all(&self) -> RpcDto {
        if self.enable_control {
            self.node.search_receivable_all();
            RpcDto::SearchReceivableAll(SuccessDto::new())
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
