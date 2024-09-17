use std::sync::Arc;
use rsnano_core::{BlockHash, UncheckedInfo, UncheckedKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{UncheckedKeysDto, UncheckedKeyDto};
use serde_json::to_string_pretty;
use std::sync::Mutex;

pub async fn unchecked_keys(node: Arc<Node>, key: BlockHash, count: u64) -> String {
    let unchecked_keys = Arc::new(Mutex::new(Vec::new()));

    node.unchecked.for_each_with_dependency(
        &key.into(),
        &mut {
            let unchecked_keys = Arc::clone(&unchecked_keys);
            move |unchecked_key: &UncheckedKey, info: &UncheckedInfo| {
                let mut unchecked_keys_guard = unchecked_keys.lock().unwrap();
                if unchecked_keys_guard.len() < count as usize {
                    if let Some(block) = info.block.as_ref() {
                        let dto = UncheckedKeyDto::new(
                            unchecked_key.hash,
                            block.hash(),
                            info.modified,
                            block.json_representation(),
                        );
                        unchecked_keys_guard.push(dto);
                    }
                }
            }
        },
        &Box::new(|| unchecked_keys.lock().unwrap().len() < count as usize)
    );

    let unchecked_keys = Arc::try_unwrap(unchecked_keys).unwrap().into_inner().unwrap();
    to_string_pretty(&UncheckedKeysDto::new(unchecked_keys)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Amount, BlockBuilder, BlockHash, DEV_GENESIS_KEY};
    use rsnano_ledger::DEV_GENESIS_HASH;
    use rsnano_node::node::Node;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, System};

    fn setup_test_environment(node: Arc<Node>) -> BlockHash {
        let genesis_hash = *DEV_GENESIS_HASH;
        let key = rsnano_core::KeyPair::new();

        // Create and process send block
        let send = BlockBuilder::legacy_send()
            .previous(genesis_hash)
            .destination(key.public_key().into())
            .balance(Amount::raw(100))
            .sign(DEV_GENESIS_KEY.clone())
            .work(node.work_generate_dev(genesis_hash.into()))
            .build();

        node.process_active(send.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send),
            "send not active on node 1",
        );

        // Create and process open block
        let open = BlockBuilder::legacy_open()
            .source(send.hash())
            .representative(key.public_key().into())
            .account(key.public_key().into())
            .sign(&key)
            .work(node.work_generate_dev(key.public_key().into()))
            .build();

        node.process(open.clone()).unwrap();
       /*assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&open),
            "open not active on node 1",
        );*/

        send.hash()
    }

    #[test]
    fn unchecked_keys() {
        let mut system = System::new();
        let node = system.make_node();

        let hash = setup_test_environment(node.clone());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.tokio.block_on(async {
            rpc_client
                .unchecked_keys(hash, 1)
                .await
                .unwrap()
        });

        println!("{:?}", result);

        server.abort();
    }
}