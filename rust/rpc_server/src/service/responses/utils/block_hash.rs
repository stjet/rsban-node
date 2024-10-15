use rsnano_core::{BlockEnum, JsonBlock};
use rsnano_rpc_messages::HashRpcMessage;
use serde_json::to_string_pretty;

pub async fn block_hash(block: JsonBlock) -> String {
    let block_enum: BlockEnum = block.into();
    to_string_pretty(&HashRpcMessage::new(block_enum.hash())).unwrap()
}
