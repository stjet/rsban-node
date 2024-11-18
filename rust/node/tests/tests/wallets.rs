use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, WalletId, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::wallets::WalletsExt;
use test_helpers::System;

#[test]
fn open_create() {
    let mut system = System::new();
    let node = system.make_node();
    assert_eq!(node.wallets.mutex.lock().unwrap().len(), 1); // it starts out with a default wallet
    let id = WalletId::random();
    assert_eq!(node.wallets.wallet_exists(&id), false);
    node.wallets.create(id);
    assert_eq!(node.wallets.wallet_exists(&id), true);
}

#[test]
fn vote_minimum() {
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();

    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - node.config.vote_minimum,
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(*DEV_GENESIS_HASH),
    ));
    node.process(send1.clone()).unwrap();

    let open1 = BlockEnum::State(StateBlock::new(
        key1.account(),
        BlockHash::zero(),
        key1.public_key(),
        node.config.vote_minimum,
        send1.hash().into(),
        &key1,
        node.work_generate_dev(&key1),
    ));
    node.process(open1.clone()).unwrap();

    // send2 with amount vote_minimum - 1 (not voting representative)
    let send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - node.config.vote_minimum * 2 + Amount::raw(1),
        key2.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send1.hash()),
    ));
    node.process(send2.clone()).unwrap();

    let open2 = BlockEnum::State(StateBlock::new(
        key2.account(),
        BlockHash::zero(),
        key2.public_key(),
        node.config.vote_minimum - Amount::raw(1),
        send2.hash().into(),
        &key2,
        node.work_generate_dev(&key2),
    ));
    node.process(open2.clone()).unwrap();

    let wallet_id = node.wallets.wallet_ids()[0];
    assert_eq!(
        node.wallets
            .mutex
            .lock()
            .unwrap()
            .get(&wallet_id)
            .unwrap()
            .representatives
            .lock()
            .unwrap()
            .len(),
        0
    );

    node.wallets
        .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), false)
        .unwrap();
    node.wallets
        .insert_adhoc2(&wallet_id, &key1.private_key(), false)
        .unwrap();
    node.wallets
        .insert_adhoc2(&wallet_id, &key2.private_key(), false)
        .unwrap();
    node.wallets.compute_reps();
    assert_eq!(
        node.wallets
            .mutex
            .lock()
            .unwrap()
            .get(&wallet_id)
            .unwrap()
            .representatives
            .lock()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn exists() {
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();
    let wallet_id = node.wallets.wallet_ids()[0];

    assert_eq!(node.wallets.exists(&key1.public_key()), false);
    assert_eq!(node.wallets.exists(&key2.public_key()), false);

    node.wallets
        .insert_adhoc2(&wallet_id, &key1.private_key(), false)
        .unwrap();
    assert_eq!(node.wallets.exists(&key1.public_key()), true);
    assert_eq!(node.wallets.exists(&key2.public_key()), false);

    node.wallets
        .insert_adhoc2(&wallet_id, &key2.private_key(), false)
        .unwrap();
    assert_eq!(node.wallets.exists(&key1.public_key()), true);
    assert_eq!(node.wallets.exists(&key2.public_key()), true);
}
