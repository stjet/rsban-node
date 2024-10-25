use rsnano_core::BlockEnum;
use rsnano_rpc_messages::{BlockHashArgs, HashRpcMessage};

pub fn block_hash(args: BlockHashArgs) -> HashRpcMessage {
    let block_enum: BlockEnum = args.block.into();
    HashRpcMessage::new(block_enum.hash())
}
