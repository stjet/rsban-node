use rsnano_core::{Amount, PublicKey, RawKey, WalletId};
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ReceivableDto, WalletReceivableArgs};
use test_helpers::{send_block_to, setup_rpc_client_and_server, System};

#[test]
fn wallet_receivable_include_only_confirmed_false() {
    let mut system = System::new();
    let node = system.make_node();

    let wallet = WalletId::zero();
    node.wallets.create(wallet);
    let private_key = RawKey::zero();
    let public_key: PublicKey = (&private_key).try_into().unwrap();
    node.wallets
        .insert_adhoc2(&wallet, &private_key, false)
        .unwrap();

    let send = send_block_to(node.clone(), public_key.into(), Amount::raw(1));

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let args = WalletReceivableArgs::builder(wallet, 1)
        .include_unconfirmed_blocks()
        .build();

    let result = node
        .runtime
        .block_on(async { rpc_client.wallet_receivable(args).await.unwrap() });

    if let ReceivableDto::Blocks { blocks } = result {
        assert_eq!(blocks.get(&public_key.into()).unwrap(), &vec![send.hash()]);
    } else {
        panic!("Expected ReceivableDto::Blocks");
    }

    server.abort();
}

#[test]
fn wallet_receivable_options_none() {
    let mut system = System::new();
    let node = system.make_node();

    let wallet = WalletId::zero();
    node.wallets.create(wallet);
    let private_key = RawKey::zero();
    let public_key: PublicKey = (&private_key).try_into().unwrap();
    node.wallets
        .insert_adhoc2(&wallet, &private_key, false)
        .unwrap();

    let send = send_block_to(node.clone(), public_key.into(), Amount::raw(1));
    node.ledger.confirm(&mut node.ledger.rw_txn(), send.hash());

    node.ledger
        .confirmed()
        .block_exists_or_pruned(&node.ledger.read_txn(), &send.hash());

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let result = node.runtime.block_on(async {
        rpc_client
            .wallet_receivable(WalletReceivableArgs::new(wallet, 1))
            .await
            .unwrap()
    });

    if let ReceivableDto::Blocks { blocks } = result {
        assert_eq!(blocks.get(&public_key.into()).unwrap(), &vec![send.hash()]);
    } else {
        panic!("Expected ReceivableDto::Blocks");
    }

    server.abort();
}

#[test]
fn wallet_receivable_threshold_some() {
    let mut system = System::new();
    let node = system.make_node();

    let wallet = WalletId::zero();
    node.wallets.create(wallet);
    let private_key = RawKey::zero();
    let public_key: PublicKey = (&private_key).try_into().unwrap();
    node.wallets
        .insert_adhoc2(&wallet, &private_key, false)
        .unwrap();

    let send = send_block_to(node.clone(), public_key.into(), Amount::raw(1));
    node.ledger.confirm(&mut node.ledger.rw_txn(), send.hash());
    let send2 = send_block_to(node.clone(), public_key.into(), Amount::raw(2));
    node.ledger.confirm(&mut node.ledger.rw_txn(), send2.hash());

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

    let args = WalletReceivableArgs::builder(wallet, 2)
        .threshold(Amount::raw(1))
        .build();

    let result = node
        .runtime
        .block_on(async { rpc_client.wallet_receivable(args).await.unwrap() });

    if let ReceivableDto::Threshold { blocks } = result {
        let account_blocks = blocks.get(&public_key.into()).unwrap();
        assert_eq!(account_blocks.len(), 2);
        assert_eq!(account_blocks.get(&send.hash()).unwrap(), &Amount::raw(1));
        assert_eq!(account_blocks.get(&send2.hash()).unwrap(), &Amount::raw(2));
    } else {
        panic!("Expected ReceivableDto::Threshold");
    }

    server.abort();
}

#[test]
fn wallet_receivable_fails_without_enable_control() {
    let mut system = System::new();
    let node = system.make_node();

    let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

    let result = node.runtime.block_on(async {
        rpc_client
            .wallet_receivable(WalletReceivableArgs::new(WalletId::zero(), 1))
            .await
    });

    assert_eq!(
        result.err().map(|e| e.to_string()),
        Some("node returned error: \"RPC control is disabled\"".to_string())
    );

    server.abort();
}