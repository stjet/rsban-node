use rsnano_core::{
    Amount, BlockEnum, BlockHash, ChangeBlock, Epoch, KeyPair, Link, OpenBlock, PublicKey,
    ReceiveBlock, SendBlock, StateBlock, DEV_GENESIS_KEY,
};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_PUB_KEY};
use rsnano_node::{
    config::{FrontiersConfirmationMode, NodeConfig},
    stats::{DetailType, Direction, StatType},
};
use test_helpers::System;

#[test]
fn single() {
    let amount = Amount::MAX;
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let latest1 = node.latest(&DEV_GENESIS_ACCOUNT);
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest1,
        *DEV_GENESIS_PUB_KEY,
        amount - Amount::raw(100),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest1),
    ));
    node.process(send1.clone()).unwrap();
    let mut tx = node.ledger.rw_txn();
    assert_eq!(
        node.ledger.confirmed().block_exists(&tx, &send1.hash()),
        false
    );
    node.ledger.confirm(&mut tx, send1.hash());
    assert_eq!(
        node.ledger.confirmed().block_exists(&tx, &send1.hash()),
        true
    );
    let conf_height = node
        .ledger
        .get_confirmation_height(&tx, &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(conf_height.height, 2);
    assert_eq!(conf_height.frontier, send1.hash());

    // Rollbacks should fail as these blocks have been cemented
    assert!(node.ledger.rollback(&mut tx, &latest1).is_err());
    assert!(node.ledger.rollback(&mut tx, &send1.hash()).is_err());
    assert_eq!(
        node.stats.count(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In
        ),
        1
    );
    assert_eq!(node.ledger.cemented_count(), 2);
}

#[test]
fn multiple_accounts() {
    let mut system = System::new();
    let cfg = NodeConfig {
        frontiers_confirmation: FrontiersConfirmationMode::Disabled,
        ..System::default_config()
    };
    let node = system.build_node().config(cfg).finish();
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();
    let key3 = KeyPair::new();
    let latest1 = node.latest(&DEV_GENESIS_ACCOUNT);

    let quorum_delta = node.online_reps.lock().unwrap().quorum_delta();

    // Send to all accounts
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest1,
        *DEV_GENESIS_PUB_KEY,
        quorum_delta + Amount::raw(300),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest1),
    ));
    let send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send1.hash(),
        *DEV_GENESIS_PUB_KEY,
        quorum_delta + Amount::raw(200),
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send1.hash()),
    ));
    let send3 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send2.hash(),
        *DEV_GENESIS_PUB_KEY,
        quorum_delta + Amount::raw(100),
        key3.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send2.hash()),
    ));

    // Open all accounts
    let open1 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - quorum_delta - Amount::raw(300),
        send1.hash().into(),
        &key1,
        node.work_generate_dev(&key1),
    ));
    let open2 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(100),
        send2.hash().into(),
        &key2,
        node.work_generate_dev(&key2),
    ));
    let open3 = BlockEnum::State(StateBlock::new(
        key3.public_key().as_account(),
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(100),
        send3.hash().into(),
        &key3,
        node.work_generate_dev(&key3),
    ));

    // Send and receive various blocks to these accounts
    let send4 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        open1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(50),
        key2.public_key().as_account().into(),
        &key1,
        node.work_generate_dev(open1.hash()),
    ));
    let send5 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        send4.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(10),
        key2.public_key().as_account().into(),
        &key1,
        node.work_generate_dev(send4.hash()),
    ));
    let receive1 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        open2.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - quorum_delta - Amount::raw(250),
        send4.hash().into(),
        &key2,
        node.work_generate_dev(open2.hash()),
    ));
    let send6 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        receive1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(10),
        key3.public_key().as_account().into(),
        &key2,
        node.work_generate_dev(receive1.hash()),
    ));
    let receive2 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        send6.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(50),
        send5.hash().into(),
        &key2,
        node.work_generate_dev(send6.hash()),
    ));
    node.process_multi(&[
        send1.clone(),
        send2.clone(),
        send3.clone(),
        open1.clone(),
        open2.clone(),
        open3.clone(),
        send4.clone(),
        send5.clone(),
        receive1.clone(),
        send6.clone(),
        receive2.clone(),
    ]);

    // Check confirmation heights of all the accounts (except genesis) are uninitialized (0),
    // as we have any just added them to the ledger and not processed any live transactions yet.
    let mut tx = node.ledger.rw_txn();
    assert_eq!(
        node.ledger
            .get_confirmation_height(&tx, &DEV_GENESIS_ACCOUNT)
            .unwrap()
            .height,
        1
    );
    assert!(node
        .ledger
        .get_confirmation_height(&tx, &key1.public_key().as_account())
        .is_none());
    assert!(node
        .ledger
        .get_confirmation_height(&tx, &key2.public_key().as_account())
        .is_none());
    assert!(node
        .ledger
        .get_confirmation_height(&tx, &key3.public_key().as_account())
        .is_none());

    // The nodes process a live receive which propagates across to all accounts
    let mut receive3 = BlockEnum::State(StateBlock::new(
        key3.public_key().as_account(),
        open3.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - quorum_delta - Amount::raw(160),
        send6.hash().into(),
        &key3,
        node.work_generate_dev(open3.hash()),
    ));
    node.ledger.process(&mut tx, &mut receive3).unwrap();
    let confirmed = node.ledger.confirm(&mut tx, receive3.hash());
    assert_eq!(confirmed.len(), 10);
    assert_eq!(
        node.stats.count(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In
        ),
        10
    );
    assert_eq!(node.ledger.cemented_count(), 11);
    assert!(node.ledger.confirmed().block_exists(&tx, &receive3.hash()));
    assert_eq!(
        node.ledger
            .any()
            .get_account(&tx, &DEV_GENESIS_ACCOUNT)
            .unwrap()
            .block_count,
        4
    );
    assert_eq!(
        node.ledger
            .get_confirmation_height(&tx, &DEV_GENESIS_ACCOUNT)
            .unwrap()
            .height,
        4
    );
    assert_eq!(
        node.ledger
            .get_confirmation_height(&tx, &DEV_GENESIS_ACCOUNT)
            .unwrap()
            .frontier,
        send3.hash()
    );
    assert_eq!(
        node.ledger
            .any()
            .get_account(&tx, &key1.public_key().as_account())
            .unwrap()
            .block_count,
        3
    );
    assert_eq!(
        node.ledger
            .get_confirmation_height(&tx, &key1.public_key().as_account())
            .unwrap()
            .height,
        2
    );

    // The accounts for key1 and key2 have 1 more block in the chain than is confirmed.
    // So this can be rolled back, but the one before that cannot. Check that this is the case
    assert!(node.ledger.rollback(&mut tx, &receive2.hash()).is_ok());
    assert!(node.ledger.rollback(&mut tx, &send5.hash()).is_ok());
    assert!(node.ledger.rollback(&mut tx, &send4.hash()).is_err());
    assert!(node.ledger.rollback(&mut tx, &send6.hash()).is_err());

    // Confirm the other latest can't be rolled back either
    assert!(node.ledger.rollback(&mut tx, &receive3.hash()).is_err());
    assert!(node.ledger.rollback(&mut tx, &send3.hash()).is_err());

    // Attempt some others which have been cemented
    assert!(node.ledger.rollback(&mut tx, &open1.hash()).is_err());
    assert!(node.ledger.rollback(&mut tx, &send2.hash()).is_err());
}

#[test]
fn send_receive_between_2_accounts() {
    let mut system = System::new();
    let cfg = NodeConfig {
        frontiers_confirmation: FrontiersConfirmationMode::Disabled,
        ..System::default_config()
    };
    let node = system.build_node().config(cfg).finish();
    let key1 = KeyPair::new();
    let key1_acc = key1.public_key().as_account();
    let latest = node.latest(&DEV_GENESIS_ACCOUNT);

    let quorum_delta = node.online_reps.lock().unwrap().quorum_delta();

    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest,
        *DEV_GENESIS_PUB_KEY,
        quorum_delta + Amount::raw(2),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest),
    ));
    let open1 = BlockEnum::State(StateBlock::new(
        key1_acc,
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - quorum_delta - Amount::raw(2),
        send1.hash().into(),
        &key1,
        node.work_generate_dev(key1_acc),
    ));
    let send2 = BlockEnum::State(StateBlock::new(
        key1_acc,
        open1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(1000),
        (*DEV_GENESIS_ACCOUNT).into(),
        &key1,
        node.work_generate_dev(open1.hash()),
    ));
    let send3 = BlockEnum::State(StateBlock::new(
        key1_acc,
        send2.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(900),
        (*DEV_GENESIS_ACCOUNT).into(),
        &key1,
        node.work_generate_dev(send2.hash()),
    ));
    let send4 = BlockEnum::State(StateBlock::new(
        key1_acc,
        send3.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(500),
        (*DEV_GENESIS_ACCOUNT).into(),
        &key1,
        node.work_generate_dev(send3.hash()),
    ));
    let receive1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1000),
        send2.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send1.hash()),
    ));
    let receive2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(900),
        send3.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive1.hash()),
    ));
    let receive3 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive2.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(500),
        send4.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive2.hash()),
    ));
    let send5 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive3.hash(),
        *DEV_GENESIS_PUB_KEY,
        quorum_delta + Amount::raw(1),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive3.hash()),
    ));
    let receive4 = BlockEnum::State(StateBlock::new(
        key1_acc,
        send4.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(500) + (Amount::MAX - Amount::raw(500) - quorum_delta - Amount::raw(1)),
        send5.hash().into(),
        &key1,
        node.work_generate_dev(send4.hash()),
    ));
    let key2 = KeyPair::new();
    let send6 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send5.hash(),
        *DEV_GENESIS_PUB_KEY,
        quorum_delta,
        key2.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send5.hash()),
    ));
    // Unpocketed send

    node.process_multi(&[
        send1.clone(),
        open1.clone(),
        send2.clone(),
        receive1.clone(),
        send3.clone(),
        send4.clone(),
        receive2.clone(),
        receive3.clone(),
        send5.clone(),
        send6.clone(),
        receive4.clone(),
    ]);

    let mut tx = node.ledger.rw_txn();
    let confirmed = node.ledger.confirm(&mut tx, receive4.hash());
    assert_eq!(confirmed.len(), 10);
    assert_eq!(
        node.stats.count(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In
        ),
        10
    );
    assert_eq!(node.ledger.cemented_count(), 11);
}

#[test]
fn send_receive_self() {
    let mut system = System::new();
    let cfg = NodeConfig {
        frontiers_confirmation: FrontiersConfirmationMode::Disabled,
        ..System::default_config()
    };
    let node = system.build_node().config(cfg).finish();
    let latest = node.latest(&DEV_GENESIS_ACCOUNT);

    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(2),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest),
    ));
    let receive1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX,
        send1.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send1.hash()),
    ));
    let send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(2),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive1.hash()),
    ));
    let send3 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send2.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(3),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send2.hash()),
    ));
    let receive2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send3.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(1),
        send2.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send3.hash()),
    ));
    let receive3 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive2.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX,
        send3.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive2.hash()),
    ));

    // Send to another account to prevent automatic receiving on the genesis account
    let key1 = KeyPair::new();
    let send4 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        receive3.hash(),
        *DEV_GENESIS_PUB_KEY,
        node.online_reps.lock().unwrap().quorum_delta(),
        key1.public_key().as_account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(receive3.hash()),
    ));

    node.process_multi(&[
        send1.clone(),
        receive1.clone(),
        send2.clone(),
        send3.clone(),
        receive2.clone(),
        receive3.clone(),
        send4.clone(),
    ]);

    let mut tx = node.ledger.rw_txn();
    let confirmed = node.ledger.confirm(&mut tx, receive3.hash());
    assert_eq!(confirmed.len(), 6);
    assert!(node.ledger.confirmed().block_exists(&tx, &receive3.hash()));
    assert_eq!(
        node.ledger
            .any()
            .get_account(&tx, &DEV_GENESIS_ACCOUNT)
            .unwrap()
            .block_count,
        8
    );
    assert_eq!(node.ledger.cemented_count(), 7);
}

#[test]
fn all_block_types() {
    let mut system = System::new();
    let cfg = NodeConfig {
        frontiers_confirmation: FrontiersConfirmationMode::Disabled,
        ..System::default_config()
    };
    let node = system.build_node().config(cfg).finish();
    let latest = node.latest(&DEV_GENESIS_ACCOUNT);
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();

    let send = BlockEnum::LegacySend(SendBlock::new(
        &latest,
        &key1.public_key().as_account(),
        &(Amount::MAX - Amount::nano(1000)),
        &DEV_GENESIS_KEY.private_key(),
        node.work_generate_dev(latest),
    ));
    let send1 = BlockEnum::LegacySend(SendBlock::new(
        &send.hash(),
        &key2.public_key().as_account(),
        &(Amount::MAX - Amount::nano(2000)),
        &DEV_GENESIS_KEY.private_key(),
        node.work_generate_dev(send.hash()),
    ));
    let open = BlockEnum::LegacyOpen(OpenBlock::new(
        send.hash(),
        *DEV_GENESIS_PUB_KEY,
        key1.public_key().as_account(),
        &key1.private_key(),
        node.work_generate_dev(&key1),
    ));
    let state_open = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        BlockHash::zero(),
        PublicKey::zero(),
        Amount::nano(1000),
        send1.hash().into(),
        &key2,
        node.work_generate_dev(&key2),
    ));
    let send2 = BlockEnum::LegacySend(SendBlock::new(
        &open.hash(),
        &key2.public_key().as_account(),
        &Amount::zero(),
        &key1.private_key(),
        node.work_generate_dev(open.hash()),
    ));
    let state_receive = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_open.hash(),
        PublicKey::zero(),
        Amount::nano(2000),
        send2.hash().into(),
        &key2,
        node.work_generate_dev(state_open.hash()),
    ));
    let state_send = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_receive.hash(),
        PublicKey::zero(),
        Amount::nano(1000),
        key1.public_key().as_account().into(),
        &key2,
        node.work_generate_dev(state_receive.hash()),
    ));
    let receive = BlockEnum::LegacyReceive(ReceiveBlock::new(
        send2.hash(),
        state_send.hash(),
        &key1.private_key(),
        node.work_generate_dev(send2.hash()),
    ));
    let change = BlockEnum::LegacyChange(ChangeBlock::new(
        receive.hash(),
        key2.public_key(),
        &key1.private_key(),
        node.work_generate_dev(receive.hash()),
    ));
    let state_change = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_send.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::nano(1000),
        Link::zero(),
        &key2,
        node.work_generate_dev(state_send.hash()),
    ));
    let epoch = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_change.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::nano(1000),
        node.ledger.epoch_link(Epoch::Epoch1).unwrap(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(state_change.hash()),
    ));
    let epoch1 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        change.hash(),
        key2.public_key(),
        Amount::nano(1000),
        node.ledger.epoch_link(Epoch::Epoch1).unwrap(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(change.hash()),
    ));
    let state_send1 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        epoch1.hash(),
        PublicKey::zero(),
        Amount::nano(999),
        key2.public_key().as_account().into(),
        &key1,
        node.work_generate_dev(epoch1.hash()),
    ));
    let state_receive2 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        epoch.hash(),
        PublicKey::zero(),
        Amount::nano(1001),
        state_send1.hash().into(),
        &key2,
        node.work_generate_dev(epoch.hash()),
    ));
    let state_send2 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_receive2.hash(),
        PublicKey::zero(),
        Amount::nano(1000),
        key1.public_key().as_account().into(),
        &key2,
        node.work_generate_dev(state_receive2.hash()),
    ));
    let state_send3 = BlockEnum::State(StateBlock::new(
        key2.public_key().as_account(),
        state_send2.hash(),
        PublicKey::zero(),
        Amount::nano(999),
        key1.public_key().as_account().into(),
        &key2,
        node.work_generate_dev(state_send2.hash()),
    ));
    let state_send4 = BlockEnum::State(StateBlock::new(
        key1.public_key().as_account(),
        state_send1.hash(),
        PublicKey::zero(),
        Amount::nano(998),
        (*DEV_GENESIS_ACCOUNT).into(),
        &key1,
        node.work_generate_dev(state_send1.hash()),
    ));
    let state_receive3 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::nano(1999),
        state_send4.hash().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send1.hash()),
    ));
    node.process_multi(&[
        send,
        send1,
        open,
        state_open,
        send2,
        state_receive,
        state_send,
        receive,
        change,
        state_change,
        epoch,
        epoch1,
        state_send1,
        state_receive2,
        state_send2.clone(),
        state_send3,
        state_send4,
        state_receive3,
    ]);
    let mut tx = node.ledger.rw_txn();
    let confirmed = node.ledger.confirm(&mut tx, state_send2.hash());
    assert_eq!(confirmed.len(), 15);
    assert_eq!(node.ledger.cemented_count(), 16);
}
