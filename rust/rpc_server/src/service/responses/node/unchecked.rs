use std::sync::{Arc, Mutex};
use rsnano_node::node::Node;
use rsnano_rpc_messages::UncheckedDto;
use rsnano_core::{UncheckedInfo, UncheckedKey};
use serde_json::to_string_pretty;
use std::collections::HashMap;

pub async fn unchecked(node: Arc<Node>, count: u64) -> String {
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
    to_string_pretty(&UncheckedDto::new(blocks)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{BlockHash, KeyPair, StateBlockBuilder};
    use test_helpers::{assert_timely_msg, System};
    use tokio::time::Duration;

    #[test]
    fn test_unchecked() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let key = KeyPair::new();

        let open = StateBlockBuilder::new()
            .account(key.account())
            .previous(BlockHash::zero())
            .representative(key.account())
            .balance(1)
            .link(key.account())
            .sign(&key)
            .work(node.work_generate_dev(key.account().into()))
            .build();

        let open2 = StateBlockBuilder::new()
            .account(key.account())
            .previous(BlockHash::zero())
            .representative(key.account())
            .balance(2)
            .link(key.account())
            .sign(&key)
            .work(node.work_generate_dev(key.account().into()))
            .build();

        node.process_active(open.clone());

        node.process_active(open2.clone());

        assert_timely_msg(
            Duration::from_secs(10),
            || node.unchecked.len() == 2,
            "Expected 2 unchecked blocks after 10 seconds",
        );

        let unchecked_dto = node.tokio.block_on(async {
            rpc_client.unchecked(2).await.unwrap()
        });

        assert_eq!(unchecked_dto.blocks.len(), 2);
        assert!(unchecked_dto.blocks.contains_key(&open.hash()));
        assert!(unchecked_dto.blocks.contains_key(&open2.hash()));

        server.abort();
    }
}