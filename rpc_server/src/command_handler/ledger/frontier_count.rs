use crate::command_handler::RpcCommandHandler;
use rsban_rpc_messages::CountResponse;

impl RpcCommandHandler {
    pub(crate) fn frontier_count(&self) -> CountResponse {
        CountResponse::new(self.node.ledger.account_count())
    }
}
