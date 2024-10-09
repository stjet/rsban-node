use rsnano_core::{Amount, DEV_GENESIS_KEY};
use rsnano_node::NodeExt;
use test_helpers::{
    get_available_port, send_block, setup_rpc_client, setup_rpc_client_and_server,
    setup_rpc_server, System, TestNode,
};

#[tokio::test]
async fn account_balance_only_confirmed_none() {
    let node = TestNode::new().await;
    node.start();

    send_block(node.clone());

    let port = get_available_port();
    setup_rpc_server(port, node.clone(), false).await;
    let rpc_client = setup_rpc_client(port);

    let result = rpc_client
        .account_balance(DEV_GENESIS_KEY.public_key().as_account(), None)
        .await
        .unwrap();

    assert_eq!(
        result.balance,
        Amount::raw(340282366920938463463374607431768211455)
    );

    assert_eq!(result.pending, Amount::zero());

    assert_eq!(result.receivable, Amount::zero());
}

#[test]
fn account_balance_only_confirmed_true() {
    let mut system = System::new();
    let node = system.make_node();

    send_block(node.clone());

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

    let result = node.runtime.block_on(async {
        rpc_client
            .account_balance(DEV_GENESIS_KEY.public_key().as_account(), Some(true))
            .await
            .unwrap()
    });

    assert_eq!(
        result.balance,
        Amount::raw(340282366920938463463374607431768211455)
    );

    assert_eq!(result.pending, Amount::zero());

    assert_eq!(result.receivable, Amount::zero());

    server.abort();
}

#[test]
fn account_balance_only_confirmed_false() {
    let mut system = System::new();
    let node = system.make_node();

    send_block(node.clone());

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

    let result = node.runtime.block_on(async {
        rpc_client
            .account_balance(DEV_GENESIS_KEY.public_key().as_account(), Some(false))
            .await
            .unwrap()
    });

    assert_eq!(
        result.balance,
        Amount::raw(340282366920938463463374607431768211454)
    );

    assert_eq!(result.pending, Amount::raw(1));

    assert_eq!(result.receivable, Amount::raw(1));

    server.abort();
}
