use crate::command_handler::RpcCommandHandler;
use rsnano_core::{BlockHash, JsonBlock};
use rsnano_rpc_messages::{BlocksDto, HashesArgs};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn blocks(&self, args: HashesArgs) -> BlocksDto {
        let mut blocks: HashMap<BlockHash, JsonBlock> = HashMap::new();
        let txn = self.node.ledger.read_txn();
        for hash in args.hashes {
            let block = self.node.ledger.get_block(&txn, &hash).unwrap();
            blocks.insert(hash, block.json_representation());
        }
        BlocksDto::new(blocks)
    }
}
