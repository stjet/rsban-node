use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::BlockCountResponse;

impl RpcCommandHandler {
    pub(crate) fn block_count(&self) -> BlockCountResponse {
        let count = self.node.ledger.block_count();
        let unchecked = self.node.unchecked.buffer_count() as u64;
        let cemented = self.node.ledger.cemented_count();
        BlockCountResponse {
            count,
            unchecked,
            cemented,
        }
    }
}
