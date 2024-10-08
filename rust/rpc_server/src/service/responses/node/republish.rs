use rsnano_core::{BlockHash, PendingKey};
use rsnano_node::node::{Node, NodeExt};
use rsnano_rpc_messages::{BlockHashesDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;
use std::time::Duration;

pub async fn republish(
    node: Arc<Node>,
    hash: BlockHash,
    sources: Option<u64>,
    destinations: Option<u64>,
    count: Option<u64>,
) -> String {
    let mut blocks = Vec::new();
    let transaction = node.store.tx_begin_read();
    let count = count.unwrap_or(1024);

    if let Some(mut block) = node.ledger.any().get_block(&transaction, &hash) {
        let mut republish_bundle = Vec::new();

        for _ in 0..count {
            if hash.is_zero() {
                break;
            }

            // Handle sources
            if let Some(sources_count) = sources {
                let source = block
                    .source_field()
                    .or_else(|| block.link_field().map(|link| link.into()))
                    .unwrap_or_default();
                let mut source_block = node.ledger.any().get_block(&transaction, &source);
                let mut source_hashes = Vec::new();

                while let Some(sb) = source_block {
                    if source_hashes.len() >= sources_count as usize {
                        break;
                    }
                    source_hashes.push(sb.hash());
                    let previous = sb.previous();
                    source_block = node.ledger.any().get_block(&transaction, &previous);
                }

                for hash in source_hashes.into_iter().rev() {
                    if let Some(b) = node.ledger.any().get_block(&transaction, &hash) {
                        republish_bundle.push(b.clone());
                        blocks.push(hash);
                    }
                }
            }

            // Add the current block
            republish_bundle.push(block.clone());
            blocks.push(hash);

            // Handle destinations
            if let Some(destinations_count) = destinations {
                if let Some(destination) = block.destination() {
                    if !node
                        .ledger
                        .any()
                        .get_pending(&transaction, &PendingKey::new(destination, hash))
                        .is_some()
                    {
                        let mut previous =
                            match node.ledger.any().account_head(&transaction, &destination) {
                                Some(block_hash) => block_hash,
                                None => {
                                    return to_string_pretty(&ErrorDto::new(
                                        "Account head not found".to_string(),
                                    ))
                                    .unwrap()
                                }
                            };
                        let mut dest_block = node.ledger.any().get_block(&transaction, &previous);
                        let mut dest_hashes = Vec::new();

                        while let Some(db) = dest_block {
                            if dest_hashes.len() >= destinations_count as usize {
                                break;
                            }
                            dest_hashes.push(previous);
                            let source = db
                                .source_field()
                                .or_else(|| {
                                    if db.is_send() {
                                        None
                                    } else {
                                        db.link_field().map(|link| link.into())
                                    }
                                })
                                .unwrap_or_default();
                            if hash == source {
                                break;
                            }
                            previous = db.previous();
                            dest_block = node.ledger.any().get_block(&transaction, &previous);
                        }

                        for hash in dest_hashes.into_iter().rev() {
                            if let Some(b) = node.ledger.any().get_block(&transaction, &hash) {
                                republish_bundle.push(b.clone());
                                blocks.push(hash);
                            }
                        }
                    }
                }
            }

            // Move to the next block
            let next_hash = node
                .ledger
                .any()
                .block_successor(&transaction, &hash)
                .unwrap_or_default();
            if let Some(next_block) = node.ledger.any().get_block(&transaction, &next_hash) {
                block = next_block;
            } else {
                break;
            }
        }

        // Flood the network with republished blocks
        node.flood_block_many(
            republish_bundle.into(),
            Box::new(|| {}),
            Duration::from_millis(25),
        );
    }

    to_string_pretty(&BlockHashesDto::new(blocks)).unwrap()
}

#[cfg(test)]
mod tests {
    use rsnano_core::{Amount, BlockBuilder, BlockHash, DEV_GENESIS_KEY};
    use rsnano_ledger::DEV_GENESIS_HASH;
    use rsnano_node::node::Node;
    use std::{sync::Arc, time::Duration};
    use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

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

        node.process_active(open.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&open),
            "open not active on node 1",
        );

        open.hash()
    }

    #[test]
    fn test_republish_send_block() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        setup_test_environment(node.clone());

        let send = node
            .ledger
            .any()
            .get_block(
                &node.store.tx_begin_read(),
                &node
                    .ledger
                    .any()
                    .block_successor(&node.store.tx_begin_read(), &*DEV_GENESIS_HASH)
                    .unwrap(),
            )
            .unwrap();

        // Test: Republish send block
        let result = node.tokio.block_on(async {
            rpc_client
                .republish(send.hash(), None, None, None)
                .await
                .unwrap()
        });

        assert_eq!(
            result.blocks.len(),
            1,
            "Expected 1 block, got {}",
            result.blocks.len()
        );
        assert_eq!(result.blocks[0], send.hash(), "Unexpected block hash");

        assert_timely_msg(
            Duration::from_secs(10),
            || {
                node.ledger
                    .any()
                    .block_exists(&node.ledger.read_txn(), &send.hash())
            },
            "send block not received by node 2",
        );

        server.abort();
    }

    #[test]
    fn test_republish_genesis_block() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        setup_test_environment(node.clone());

        // Test: Republish genesis block with count 1
        let result = node.tokio.block_on(async {
            rpc_client
                .republish(*DEV_GENESIS_HASH, None, None, Some(1))
                .await
                .unwrap()
        });

        assert_eq!(
            result.blocks.len(),
            1,
            "Expected 1 block, got {}",
            result.blocks.len()
        );
        assert_eq!(result.blocks[0], *DEV_GENESIS_HASH, "Unexpected block hash");

        server.abort();
    }

    #[test]
    fn test_republish_open_block_with_sources() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let block_hash = setup_test_environment(node.clone());

        //let genesis_successor = node.ledger.any().block_successor(&node.store.tx_begin_read(), &DEV_GENESIS_HASH).unwrap();
        //let send_successor = node.ledger.any().block_successor(&node.store.tx_begin_read(), &genesis_successor).unwrap();
        //let open = node.ledger.any().get_block(&node.store.tx_begin_read(), &send_successor).unwrap();

        // Test: Republish open block with sources 2
        let result = node.tokio.block_on(async {
            rpc_client
                .republish(block_hash, Some(2), None, None)
                .await
                .unwrap()
        });

        assert_eq!(
            result.blocks.len(),
            3,
            "Expected 3 blocks, got {}",
            result.blocks.len()
        );
        assert_eq!(
            result.blocks[0], *DEV_GENESIS_HASH,
            "Unexpected genesis block hash"
        );
        assert_eq!(
            result.blocks[1],
            node.ledger
                .any()
                .block_successor(&node.store.tx_begin_read(), &*DEV_GENESIS_HASH)
                .unwrap(),
            "Unexpected send block hash"
        );
        assert_eq!(result.blocks[2], block_hash, "Unexpected open block hash");

        server.abort();
    }
}
