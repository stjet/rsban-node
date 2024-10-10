use rsnano_core::{KeyPair, Amount, BlockEnum, StateBlock, WalletId, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::{Node, wallets::WalletsExt};
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

#[test]
fn chain() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let wallet_id = WalletId::zero();
    node.wallets.create(wallet_id);
    node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true);

    let genesis = node.latest(&*DEV_GENESIS_ACCOUNT);
    assert!(!genesis.is_zero());

    let key = KeyPair::new();
    let block = node.wallets.send_action2(&wallet_id, *DEV_GENESIS_ACCOUNT, key.account(), Amount::raw(1), 0, true, None).unwrap();

    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&block),
        "block not active on node",
    );

    let result = node.runtime.block_on(async {
        rpc_client
            .chain(block.hash(), u64::MAX, None, None)
            .await
            .unwrap()
    });

    let blocks = result.blocks.clone();

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0], block.hash());
    assert_eq!(blocks[1], genesis);

    server.abort();
}

#[test]
fn chain_limit() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let wallet_id = WalletId::zero();
    node.wallets.create(wallet_id);
    node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true);

    let genesis = node.latest(&*DEV_GENESIS_ACCOUNT);
    assert!(!genesis.is_zero());

    let key = KeyPair::new();
    let block = node.wallets.send_action2(&wallet_id, *DEV_GENESIS_ACCOUNT, key.account(), Amount::raw(1), 0, true, None).unwrap();

    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&block),
        "block not active on node",
    );

    let result = node.runtime.block_on(async {
        rpc_client
            .chain(block.hash(), 1, None, None) // Set count to 1
            .await
            .unwrap()
    });

    let blocks = result.blocks.clone();

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0], block.hash());

    server.abort();
}

#[test]
fn chain_offset() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let wallet_id = WalletId::zero();
    node.wallets.create(wallet_id);
    node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true);

    let genesis = node.latest(&*DEV_GENESIS_ACCOUNT);
    assert!(!genesis.is_zero());

    let key = KeyPair::new();
    let block = node.wallets.send_action2(&wallet_id, *DEV_GENESIS_ACCOUNT, key.account(), Amount::raw(1), 0, true, None).unwrap();

    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&block),
        "block not active on node",
    );

    let result = node.runtime.block_on(async {
        rpc_client
            .chain(block.hash(), u64::MAX, None, Some(1)) // Set offset to 1
            .await
            .unwrap()
    });

    let blocks = result.blocks.clone();

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0], genesis);

    server.abort();
}
