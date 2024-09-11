use std::sync::Arc;
use rsnano_core::BlockHash;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, UncheckedGetDto};
use serde_json::to_string_pretty;

pub async fn unchecked_get(node: Arc<Node>, hash: BlockHash) -> String {
    let unchecked_blocks = node.unchecked.get(&hash.into());
    
    unchecked_blocks.into_iter().next().map(|info| {
        let modified_timestamp = info.modified;
        let block = info.block.unwrap();
        let contents = block.json_representation();
        
        to_string_pretty(&UncheckedGetDto::new(modified_timestamp, contents)).unwrap()
    }).unwrap_or_else(|| to_string_pretty(&ErrorDto::new("Block not found".to_string())).unwrap())
}