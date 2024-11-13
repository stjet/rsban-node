use rsnano_core::{
    Amount, BlockEnum, Epoch, KeyPair, Signature, StateBlock, Vote, VoteCode, VoteSource, WalletId,
    DEV_GENESIS_KEY, GXRB_RATIO,
};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::wallets::WalletsExt;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use test_helpers::{assert_timely, make_fake_channel, start_election, upgrade_epoch, System};

#[test]
fn check_signature() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.online_weight_minimum = Amount::MAX;
    let node = system.build_node().config(config).finish();
    let key1 = KeyPair::new();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(100),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node.process(send1.clone()).unwrap();
    let election1 = start_election(&node, &send1.hash());
    assert_eq!(1, election1.vote_count());
    let mut vote1 = Vote::new(&DEV_GENESIS_KEY, Vote::TIMESTAMP_MIN, 0, vec![send1.hash()]);
    let good_signature = vote1.signature;
    vote1.signature = Signature::new();
    let channel = make_fake_channel(&node);
    assert_eq!(
        VoteCode::Invalid,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            channel.channel_id(),
            VoteSource::Live
        )
    );

    vote1.signature = good_signature;
    assert_eq!(
        VoteCode::Vote,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            channel.channel_id(),
            VoteSource::Live
        )
    );
    assert_eq!(
        VoteCode::Replay,
        node.vote_processor.vote_blocking(
            &Arc::new(vote1.clone()),
            channel.channel_id(),
            VoteSource::Live
        )
    );
}

// Lower timestamps are ignored
#[test]
fn add_old() {
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node.process(send1.clone()).unwrap();
    start_election(&node, &send1.hash());
    assert_timely(Duration::from_secs(5), || {
        node.active.election(&send1.qualified_root()).is_some()
    });
    let election1 = node.active.election(&send1.qualified_root()).unwrap();
    let vote1 = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN * 2,
        0,
        vec![send1.hash()],
    ));
    let channel = make_fake_channel(&node);
    node.vote_processor
        .vote_blocking(&vote1, channel.channel_id(), VoteSource::Live);

    let key2 = KeyPair::new();
    let send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key2.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    let vote2 = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN * 1,
        0,
        vec![send2.hash()],
    ));

    election1
        .mutex
        .lock()
        .unwrap()
        .last_votes
        .get_mut(&DEV_GENESIS_PUB_KEY)
        .unwrap()
        .time = SystemTime::now() - Duration::from_secs(20);
    node.vote_processor
        .vote_blocking(&vote2, channel.channel_id(), VoteSource::Live);
    assert_eq!(2, election1.vote_count());
    let votes = election1.mutex.lock().unwrap().last_votes.clone();
    assert!(votes.contains_key(&DEV_GENESIS_PUB_KEY));
    assert_eq!(send1.hash(), votes.get(&DEV_GENESIS_PUB_KEY).unwrap().hash);
    assert_eq!(send1.hash(), election1.winner_hash().unwrap());
}

// The voting cooldown is respected
#[test]
fn add_cooldown() {
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    node.process(send1.clone()).unwrap();
    start_election(&node, &send1.hash());
    assert_timely(Duration::from_secs(5), || {
        node.active.election(&send1.qualified_root()).is_some()
    });
    let election1 = node.active.election(&send1.qualified_root()).unwrap();
    let vote1 = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN * 1,
        0,
        vec![send1.hash()],
    ));
    let channel = make_fake_channel(&node);
    node.vote_processor
        .vote_blocking(&vote1, channel.channel_id(), VoteSource::Live);

    let key2 = KeyPair::new();
    let send2 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::zero(),
        key2.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));
    let vote2 = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN * 2,
        0,
        vec![send2.hash()],
    ));

    node.vote_processor
        .vote_blocking(&vote2, channel.channel_id(), VoteSource::Live);
    assert_eq!(2, election1.vote_count());
    let votes = election1.mutex.lock().unwrap().last_votes.clone();
    assert!(votes.contains_key(&DEV_GENESIS_PUB_KEY));
    assert_eq!(send1.hash(), votes.get(&DEV_GENESIS_PUB_KEY).unwrap().hash);
    assert_eq!(send1.hash(), election1.winner_hash().unwrap());
}

// Assuming necessary imports and module declarations are present
#[test]
fn vote_generator_cache() {
    let mut system = System::new();
    let node = system.make_node();

    let epoch1 = upgrade_epoch(node.clone(), Epoch::Epoch1);
    let wallet_id = WalletId::random();

    node.wallets.create(wallet_id);
    node.wallets
        .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true)
        .unwrap();

    node.vote_generators
        .generate_non_final_vote(&epoch1.root(), &epoch1.hash());

    // Wait until the votes are available
    assert_timely(Duration::from_secs(1), || {
        !node
            .history
            .votes(&epoch1.root(), &epoch1.hash(), false)
            .is_empty()
    });

    let votes = node.history.votes(&epoch1.root(), &epoch1.hash(), false);
    assert!(!votes.is_empty());

    let hashes = &votes[0].hashes;
    assert!(hashes.contains(&epoch1.hash()));
}

#[test]
fn vote_generator_multiple_representatives() {
    let mut system = System::new();
    let node = system.make_node();
    let wallet_id = WalletId::random();
    node.wallets.create(wallet_id);
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();
    let key3 = KeyPair::new();

    // Insert keys into the wallet
    node.wallets
        .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true)
        .unwrap();
    node.wallets
        .insert_adhoc2(&wallet_id, &key1.private_key(), true)
        .unwrap();
    node.wallets
        .insert_adhoc2(&wallet_id, &key2.private_key(), true)
        .unwrap();
    node.wallets
        .insert_adhoc2(&wallet_id, &key3.private_key(), true)
        .unwrap();

    let amount = Amount::raw(100 * *GXRB_RATIO);
    node.wallets.send_sync(
        wallet_id,
        *DEV_GENESIS_ACCOUNT,
        key1.account(),
        amount,
        0,
        true,
        None,
    );
    node.wallets.send_sync(
        wallet_id,
        *DEV_GENESIS_ACCOUNT,
        key2.account(),
        amount,
        0,
        true,
        None,
    );
    node.wallets.send_sync(
        wallet_id,
        *DEV_GENESIS_ACCOUNT,
        key3.account(),
        amount,
        0,
        true,
        None,
    );

    // Assert balances
    assert_timely(Duration::from_secs(3), || {
        node.balance(&key1.account()) == amount
            && node.balance(&key2.account()) == amount
            && node.balance(&key3.account()) == amount
    });

    // Change representatives
    node.wallets
        .change_action2(&wallet_id, key1.account(), key1.public_key(), 0, true);
    node.wallets
        .change_action2(&wallet_id, key2.account(), key2.public_key(), 0, true);
    node.wallets
        .change_action2(&wallet_id, key3.account(), key3.public_key(), 0, true);

    assert_eq!(node.ledger.weight(&key1.public_key()), amount);
    assert_eq!(node.ledger.weight(&key2.public_key()), amount);
    assert_eq!(node.ledger.weight(&key3.public_key()), amount);

    node.wallets.compute_reps();
    assert_eq!(node.wallets.voting_reps_count(), 4);

    let hash = node.wallets.send_sync(
        wallet_id,
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_ACCOUNT,
        Amount::raw(1),
        0,
        true,
        None,
    );
    let send = node.block(&hash).unwrap();

    // Wait until the votes are available
    assert_timely(Duration::from_secs(5), || {
        node.history.votes(&send.root(), &send.hash(), false).len() == 4
    });

    let votes = node.history.votes(&send.root(), &send.hash(), false);
    for account in &[
        key1.public_key(),
        key2.public_key(),
        key3.public_key(),
        DEV_GENESIS_KEY.public_key(),
    ] {
        let existing = votes.iter().find(|vote| vote.voting_account == *account);
        assert!(existing.is_some());
    }
}
