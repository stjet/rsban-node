use std::sync::{Arc, Mutex};
use rsnano_core::{BlockHash, HashOrAccount, UncheckedInfo, UncheckedKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{UncheckedKeysDto, UncheckedKeyDto};
use serde_json::to_string_pretty;

pub async fn unchecked_keys(node: Arc<Node>, key: HashOrAccount, count: u64) -> String {
    let unchecked_keys = Arc::new(Mutex::new(Vec::new()));

    node.unchecked.for_each_with_dependency(
        &key,
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
    use rsnano_core::{BlockHash, KeyPair, StateBlockBuilder};
    use test_helpers::{assert_timely_msg, System};
    use tokio::time::Duration;

    #[test]
    fn test_unchecked_keys() {
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
            rpc_client.unchecked_keys(key.account().into(), 2).await.unwrap()
        });

        assert_eq!(unchecked_dto.unchecked.len(), 2);
        assert!(unchecked_dto.unchecked[0].hash == open.hash());
        assert!(unchecked_dto.unchecked[1].hash == open2.hash());

        server.abort();
    }
}