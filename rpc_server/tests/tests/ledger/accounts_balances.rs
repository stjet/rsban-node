use rsban_core::{Amount, UnsavedBlockLatticeBuilder, DEV_GENESIS_KEY};
use rsban_ledger::DEV_GENESIS_ACCOUNT;
use rsban_node::Node;
use rsban_rpc_messages::AccountsBalancesArgs;
use std::sync::Arc;
use std::time::Duration;
use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

fn send_block(node: Arc<Node>) {
    let mut lattice = UnsavedBlockLatticeBuilder::new();
    let send = lattice.genesis().send(&*DEV_GENESIS_KEY, 1);

    node.process_active(send.clone());
    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&send),
        "not active on node 1",
    );
}

#[test]
fn accounts_balances_only_confirmed_none() {
    let mut system = System::new();
    let node = system.make_node();

    send_block(node.clone());

    let server = setup_rpc_client_and_server(node.clone(), false);

    let result = node.runtime.block_on(async {
        server
            .client
            .accounts_balances(vec![DEV_GENESIS_KEY.public_key().as_account()])
            .await
            .unwrap()
    });

    let account = result.balances.get(&DEV_GENESIS_ACCOUNT).unwrap();

    assert_eq!(
        account.balance,
        Amount::raw(340282366920938463463374607431768211455)
    );
    assert_eq!(account.pending, Amount::zero());
    assert_eq!(account.receivable, Amount::zero());
}

#[test]
fn account_balance_only_confirmed_true() {
    let mut system = System::new();
    let node = system.make_node();

    send_block(node.clone());

    let server = setup_rpc_client_and_server(node.clone(), false);

    let result = node.runtime.block_on(async {
        server
            .client
            .accounts_balances(vec![DEV_GENESIS_KEY.public_key().as_account()])
            .await
            .unwrap()
    });

    let account = result.balances.get(&DEV_GENESIS_ACCOUNT).unwrap();

    assert_eq!(
        account.balance,
        Amount::raw(340282366920938463463374607431768211455)
    );

    assert_eq!(account.pending, Amount::zero());
    assert_eq!(account.receivable, Amount::zero());
}

#[test]
fn account_balance_only_confirmed_false() {
    let mut system = System::new();
    let node = system.make_node();

    send_block(node.clone());

    let server = setup_rpc_client_and_server(node.clone(), false);

    let args = AccountsBalancesArgs::new(vec![DEV_GENESIS_KEY.public_key().as_account()])
        .include_unconfirmed_blocks()
        .finish();

    let result = node
        .runtime
        .block_on(async { server.client.accounts_balances(args).await.unwrap() });

    let account = result.balances.get(&DEV_GENESIS_ACCOUNT).unwrap();

    assert_eq!(
        account.balance,
        Amount::raw(340282366920938463463374607431768211454)
    );

    assert_eq!(account.pending, Amount::raw(1));
    assert_eq!(account.receivable, Amount::raw(1));
}
