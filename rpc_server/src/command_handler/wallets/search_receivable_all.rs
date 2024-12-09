use crate::command_handler::RpcCommandHandler;
use rsban_node::wallets::WalletsExt;
use rsban_rpc_messages::SuccessResponse;

impl RpcCommandHandler {
    pub(crate) fn search_receivable_all(&self) -> SuccessResponse {
        self.node.wallets.search_receivable_all();
        SuccessResponse::new()
    }
}
