use rsban_core::{Block, BlockHash, UnsavedBlockLatticeBuilder, DEV_GENESIS_KEY};
use rsban_node::Node;
use rsban_rpc_messages::ReceivableExistsArgs;
use std::sync::Arc;
use std::time::Duration;
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
fn receivable_exists_confirmed() {
    let mut system = System::new();
    let node = system.make_node();

    let send = send_block(node.clone());
    node.confirm(send.hash().clone());

    let server = setup_rpc_client_and_server(node.clone(), false);

    let result = node
        .runtime
        .block_on(async { server.client.receivable_exists(send.hash()).await.unwrap() });

    assert_eq!(result.exists, true.into());
}

#[test]
fn test_receivable_exists_unconfirmed() {
    let mut system = System::new();
    let node = system.make_node();

    let send = send_block(node.clone());

    let server = setup_rpc_client_and_server(node.clone(), false);

    let args = ReceivableExistsArgs::build(send.hash())
        .include_active()
        .include_unconfirmed_blocks()
        .finish();

    let result = node
        .runtime
        .block_on(async { server.client.receivable_exists(args).await.unwrap() });

    assert_eq!(result.exists, true.into());
}

#[test]
fn test_receivable_exists_non_existent() {
    let mut system = System::new();
    let node = system.make_node();

    let server = setup_rpc_client_and_server(node.clone(), false);

    let non_existent_hash = BlockHash::zero();
    let result = node
        .runtime
        .block_on(async { server.client.receivable_exists(non_existent_hash).await })
        .unwrap_err();

    assert!(result.to_string().contains("Block not found"));
}
