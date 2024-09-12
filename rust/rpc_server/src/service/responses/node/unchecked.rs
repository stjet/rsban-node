use std::sync::{Arc, Mutex};
use rsnano_node::node::Node;
use rsnano_rpc_messages::UncheckedDto;
use rsnano_core::{UncheckedInfo, UncheckedKey};
use std::collections::HashMap;

pub async fn unchecked(node: Arc<Node>, count: u64) -> UncheckedDto {
    let mut blocks = HashMap::new();

    node.unchecked.for_each(
        Box::new( |key: &UncheckedKey, info: &UncheckedInfo| {
            if blocks.len() < count as usize {
                if let Some(block) = info.block.as_ref() {
                    let hash = block.hash();
                    let json_block = block.json_representation();
                    blocks.insert(hash, json_block);
                }
            }
        }),
        Box::new(|| true)  // Always return true to keep iterating
    );

    UncheckedDto::new(blocks)
}
