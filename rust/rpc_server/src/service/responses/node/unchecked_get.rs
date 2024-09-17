use std::sync::{Arc, Mutex};
use rsnano_core::{BlockHash, UncheckedInfo, UncheckedKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, UncheckedGetDto};
use serde_json::to_string_pretty;

pub async fn unchecked_get(node: Arc<Node>, hash: BlockHash) -> String {
    let result = Arc::new(Mutex::new(None));
    
    node.unchecked.for_each(
        {
            let result = Arc::clone(&result);
            Box::new(move |key: &UncheckedKey, info: &UncheckedInfo| {
                if key.hash == hash {
                    let modified_timestamp = info.modified;
                    if let Some(block) = info.block.as_ref() {
                        let contents = block.json_representation();
                        let mut result_guard = result.lock().unwrap();
                        *result_guard = Some(UncheckedGetDto::new(modified_timestamp, contents));
                    }
                }
            })
        },
        Box::new(|| true)
    );

    let result = result.lock().unwrap().take();
    result.map_or_else(
        || to_string_pretty(&ErrorDto::new("Block not found".to_string())).unwrap(),
        |dto| to_string_pretty(&dto).unwrap()
    )
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Amount, BlockHash, JsonBlock, KeyPair, StateBlockBuilder};
    use test_helpers::{assert_timely_msg, System};
    use tokio::time::Duration;

    #[test]
    fn unchecked_get() {
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

        node.process_active(open.clone());

        assert_timely_msg(
            Duration::from_secs(10),
            || node.unchecked.len() == 1,
            "Expected 1 unchecked block after 10 seconds",
        );

        let unchecked_dto = node.tokio.block_on(async {
            rpc_client.unchecked_get(open.hash()).await.unwrap()
        });

        // Check that the timestamp is less than or equal to the current time
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(unchecked_dto.modified_timestamp <= current_timestamp);

        // Convert the contents to a JsonBlock
        let json_block: JsonBlock = unchecked_dto.contents;

        // Assert that it's a state block
        assert!(matches!(json_block, JsonBlock::State(_)));

        if let JsonBlock::State(state_block) = json_block {
            // Add assertions for state block fields
            assert_eq!(state_block.account, key.account());
            assert_eq!(state_block.previous, BlockHash::zero());
            assert_eq!(state_block.representative, key.account());
            assert_eq!(state_block.balance, Amount::raw(1));
            assert_eq!(state_block.link, key.account().into());
        } else {
            panic!("Expected a state block");
        }

        server.abort();
    }
}