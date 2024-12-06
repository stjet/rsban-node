use rsnano_core::{Amount, UnsavedBlockLatticeBuilder, WalletId, DEV_GENESIS_KEY};
use rsnano_ledger::BlockStatus;
use rsnano_node::{wallets::WalletsExt, Node};
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely_eq, setup_rpc_client_and_server, System};

#[test]
fn search_receivable_all() {
    let mut system = System::new();
    let node: Arc<Node> = system.make_node();

    let server = setup_rpc_client_and_server(node.clone(), true);

    let wallet_id = WalletId::zero();
    node.wallets.create(wallet_id);
    node.wallets
        .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.raw_key(), false)
        .unwrap();

    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let send = lattice
        .genesis()
        .send(&*DEV_GENESIS_KEY, node.config.receive_minimum);

    assert_eq!(node.process_local(send).unwrap(), BlockStatus::Progress);

    node.runtime.block_on(async {
        server.client.search_receivable_all().await.unwrap();
    });

    assert_timely_eq(
        Duration::from_secs(10),
        || node.balance(&DEV_GENESIS_KEY.account()),
        Amount::MAX,
    );
}

#[test]
fn search_receivable_all_fails_without_enable_control() {
    let mut system = System::new();
    let node = system.make_node();

    let server = setup_rpc_client_and_server(node.clone(), false);

    let result = node
        .runtime
        .block_on(async { server.client.search_receivable_all().await });

    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("node returned error: \"RPC control is disabled\"".to_string())
    );
}
