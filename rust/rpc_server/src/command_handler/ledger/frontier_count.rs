use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{CountRpcMessage, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn frontier_count(&self) -> RpcDto {
        RpcDto::FrontierCount(CountRpcMessage::new(self.node.ledger.account_count()))
    }
}
