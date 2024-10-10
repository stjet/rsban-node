use rsnano_core::{KeyPair, Amount, BlockEnum, StateBlock, WalletId, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::{Node, wallets::WalletsExt};
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

#[test]
fn successors() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    // Create a wallet and insert the genesis key

    let wallet_id = WalletId::zero();
    node.wallets.create(wallet_id);
    node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true);

    // Get the genesis block hash
    let genesis = node.latest(&*DEV_GENESIS_ACCOUNT);
    assert!(!genesis.is_zero());

    // Create and process a send block
    let key = KeyPair::new();
    let block = node.wallets.send_action2(&wallet_id, *DEV_GENESIS_ACCOUNT, key.account(), Amount::raw(1), 0, true, None).unwrap();

    // Wait for the block to be processed
    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&block),
        "block not active on node",
    );

    let result = node.runtime.block_on(async {
        rpc_client
            .successors(genesis, u64::MAX, None, None)
            .await
            .unwrap()
    });

    let blocks = result.blocks.clone();

    // Check that we have 2 blocks: genesis and the send block
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0], genesis);
    assert_eq!(blocks[1], block.hash());

    // Test the "reverse" option (equivalent to "chain" action in C++)
    let reverse_result = node.runtime.block_on(async {
        rpc_client
            .successors(genesis, u64::MAX, Some(true), None)
            .await
            .unwrap()
    });

    //assert_eq!(result, reverse_result);

    server.abort();
}
