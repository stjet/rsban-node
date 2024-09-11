use std::sync::{Arc, Mutex};
use rsnano_node::node::Node;
use rsnano_rpc_messages::UncheckedDto;
use rsnano_core::{UncheckedInfo, UncheckedKey};
use std::collections::HashMap;

pub async fn unchecked(node: Arc<Node>, count: u64) -> UncheckedDto {
    let blocks = Arc::new(Mutex::new(HashMap::new()));

    node.unchecked.for_each(
        {
            let blocks = Arc::clone(&blocks);
            Box::new(move |key: &UncheckedKey, info: &UncheckedInfo| {
                let mut blocks_guard = blocks.lock().unwrap();
                if blocks_guard.len() < count as usize {
                    if let Some(block) = info.block.as_ref() {
                        let hash = block.hash();
                        let json_block = block.json_representation();
                        blocks_guard.insert(hash, json_block);
                    }
                }
            })
        },
        Box::new(|| true)
    );

    let blocks = Arc::try_unwrap(blocks).unwrap().into_inner().unwrap();
    UncheckedDto::new(blocks)
}