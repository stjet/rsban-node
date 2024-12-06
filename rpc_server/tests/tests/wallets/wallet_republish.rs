use rsnano_core::{Block, UnsavedBlockLatticeBuilder, WalletId, DEV_GENESIS_KEY};
use rsnano_node::{wallets::WalletsExt, Node};
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

fn send_block(node: Arc<Node>) -> Block {
    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let send1 = lattice.genesis().send(&*DEV_GENESIS_KEY, 1);
    node.process_active(send1.clone());
    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&send1),
        "not active on node 1",
    );

    send1
}

#[test]
fn wallet_republish() {
    let mut system = System::new();
    let node = system.make_node();

    let send = send_block(node.clone());

    let server = setup_rpc_client_and_server(node.clone(), true);

    let wallet = WalletId::zero();

    node.wallets.create(wallet);

    node.wallets
        .insert_adhoc2(&wallet, &DEV_GENESIS_KEY.raw_key(), false)
        .unwrap();

    let result = node
        .runtime
        .block_on(async { server.client.wallet_republish(wallet, 1).await.unwrap() });

    assert!(
        result.blocks.len() == 1,
        "Expected 1 block, got {}",
        result.blocks.len()
    );
    assert_eq!(result.blocks[0], send.hash(), "Unexpected block hash");
}

#[test]
fn wallet_republish_fails_without_enable_control() {
    let mut system = System::new();
    let node = system.make_node();

    let server = setup_rpc_client_and_server(node.clone(), false);

    let result = node
        .runtime
        .block_on(async { server.client.wallet_republish(WalletId::zero(), 1).await });

    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("node returned error: \"RPC control is disabled\"".to_string())
    );
}

#[test]
fn wallet_republish_fails_with_wallet_not_found() {
    let mut system = System::new();
    let node = system.make_node();

    let server = setup_rpc_client_and_server(node.clone(), true);

    let result = node
        .runtime
        .block_on(async { server.client.wallet_republish(WalletId::zero(), 1).await });

    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("node returned error: \"Wallet not found\"".to_string())
    );
}
