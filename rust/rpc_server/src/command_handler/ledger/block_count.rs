use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{BlockCountDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn block_count(&self) -> RpcDto {
        let count = self.node.ledger.block_count();
        let unchecked = self.node.unchecked.buffer_count() as u64;
        let cemented = self.node.ledger.cemented_count();
        let block_count = BlockCountDto::new(count, unchecked, cemented);
        RpcDto::BlockCount(block_count)
    }
}
