use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::Node;
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

fn setup_test_environment(node: Arc<Node>) -> (KeyPair, BlockEnum, BlockEnum) {
    let keys = KeyPair::new();
    let genesis_balance = Amount::MAX;
    let send_amount = genesis_balance - Amount::raw(100);
    let remaining_balance = genesis_balance - send_amount;

    let send_block = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        remaining_balance,
        (*DEV_GENESIS_ACCOUNT).into(),
        &keys,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    node.process_active(send_block.clone());

    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&send_block),
        "send block not active",
    );

    let open_block = BlockEnum::State(StateBlock::new(
        keys.account(),
        *DEV_GENESIS_HASH,
        (*DEV_GENESIS_ACCOUNT).into(),
        send_amount,
        keys.account().into(),
        &keys,
        node.work_generate_dev(keys.account().into()),
    ));

    node.process_active(open_block.clone());

    assert_timely_msg(
        Duration::from_secs(5),
        || node.active.active(&open_block),
        "open block not active",
    );

    (keys, send_block, open_block)
}

#[test]
fn test_ledger() {
    let mut system = System::new();
    let node = system.build_node().finish();
    let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

    let key = KeyPair::new();
    let rep_weight = Amount::MAX - Amount::raw(100);

    let send = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - rep_weight,
        key.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    let status = node.process_local(send.clone()).unwrap();
    assert_eq!(status, BlockStatus::Progress);

    let open = BlockEnum::State(StateBlock::new(
        key.account(),
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        rep_weight,
        send.hash().into(),
        &key,
        node.work_generate_dev(key.public_key().into()),
    ));

    let status = node.process_local(open.clone()).unwrap();
    assert_eq!(status, BlockStatus::Progress);

    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Basic ledger test
    let result = node.runtime.block_on(async {
        rpc_client
            .ledger(
                None,       // account
                Some(1),    // count
                None,       // representative
                None,       // weight
                None,       // receivable
                None,       // modified_since
                Some(true), // sorting
                None,       // threshold
            )
            .await
            .unwrap()
    });

    let accounts = result.accounts;
    assert_eq!(accounts.len(), 1);

    for (account, info) in accounts {
        // ASSERT_EQ (key.pub.to_account (), account_text);
        assert_eq!(key.account(), account);

        // ASSERT_EQ (open->hash ().to_string (), frontier);
        assert_eq!(open.hash(), info.frontier);

        // ASSERT_EQ (open->hash ().to_string (), open_block);
        assert_eq!(open.hash(), info.open_block);

        // ASSERT_EQ (open->hash ().to_string (), representative_block);
        assert_eq!(open.hash(), info.representative_block);

        // ASSERT_EQ (send_amount.convert_to<std::string> (), balance_text);
        assert_eq!(rep_weight, info.balance);

        // ASSERT_LT (std::abs ((long)time - stol (modified_timestamp)), 5);
        assert!(((time as i64) - (info.modified_timestamp as i64)).abs() < 5);

        // ASSERT_EQ ("1", block_count);
        assert_eq!(1, info.block_count);

        // ASSERT_FALSE (weight.is_initialized ());
        assert!(info.weight.is_none());

        // ASSERT_FALSE (pending.is_initialized ());
        assert!(info.pending.is_none());

        // ASSERT_FALSE (representative.is_initialized ());
        assert!(info.representative.is_none());
    }
}

#[test]
fn test_ledger_threshold() {
    let mut system = System::new();
    let node = system.build_node().finish();
    let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

    let (keys, _, _) = setup_test_environment(node.clone());

    let genesis_balance = Amount::MAX;
    let result = node.runtime.block_on(async {
        rpc_client
            .ledger(
                None,                                   // account
                Some(2),                                // count
                None,                                   // representative
                None,                                   // weight
                None,                                   // receivable
                None,                                   // modified_since
                Some(true),                             // sorting
                Some(genesis_balance + Amount::raw(1)), // threshold
            )
            .await
            .unwrap()
    });

    let accounts = result.accounts;
    assert_eq!(accounts.len(), 1);
    assert!(accounts.contains_key(&keys.account()));
}

#[test]
fn test_ledger_pending() {
    let mut system = System::new();
    let node = system.build_node().finish();
    let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

    let (keys, send_block, _) = setup_test_environment(node.clone());

    let send_amount = Amount::MAX - Amount::raw(100);
    let send2_amount = Amount::raw(50);
    let new_remaining_balance = Amount::MAX - send_amount - send2_amount;

    let send2_block = StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send_block.hash(),
        keys.account().into(),
        new_remaining_balance,
        (*DEV_GENESIS_ACCOUNT).into(),
        &keys,
        node.work_generate_dev(keys.account().into()),
    );

    node.process_active(BlockEnum::State(send2_block.clone()));

    let result = node.runtime.block_on(async {
        rpc_client
            .ledger(
                None,                             // account
                Some(2),                          // count
                None,                             // representative
                None,                             // weight
                None,                             // receivable
                None,                             // modified_since
                Some(true),                       // sorting
                Some(send_amount + send2_amount), // threshold
            )
            .await
            .unwrap()
    });

    let accounts = result.accounts;
    assert_eq!(accounts.len(), 1);
    let account_info = accounts.get(&keys.account()).unwrap();
    assert_eq!(account_info.balance, send_amount);
    assert_eq!(account_info.pending, Some(send2_amount));
}
