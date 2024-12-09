use rsban_core::Block;
use rsban_rpc_messages::{BlockHashArgs, HashRpcMessage};

pub fn block_hash(args: BlockHashArgs) -> HashRpcMessage {
    let block_enum: Block = args.block.into();
    HashRpcMessage::new(block_enum.hash())
}
