use test_helpers::{send_block, setup_rpc_client_and_server, System};

#[test]
fn confirmation_active() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

    send_block(node.clone());

    let result = node
        .runtime
        .block_on(async { rpc_client.confirmation_active(None).await.unwrap() });

    assert!(!result.confirmations.is_empty());
    assert_eq!(result.confirmed, 0);
    assert_eq!(result.unconfirmed, 1);

    server.abort();
}