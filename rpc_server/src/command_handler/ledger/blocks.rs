use crate::command_handler::RpcCommandHandler;
use anyhow::anyhow;
use rsban_core::{BlockHash, JsonBlock};
use rsban_rpc_messages::{BlocksResponse, HashesArgs};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn blocks(&self, args: HashesArgs) -> anyhow::Result<BlocksResponse> {
        let mut blocks: HashMap<BlockHash, JsonBlock> = HashMap::new();
        let txn = self.node.ledger.read_txn();
        for hash in args.hashes {
            let block = self
                .node
                .ledger
                .get_block(&txn, &hash)
                .ok_or_else(|| anyhow!(Self::BLOCK_NOT_FOUND))?;
            blocks.insert(hash, block.json_representation());
        }
        Ok(BlocksResponse::new(blocks))
    }
}
