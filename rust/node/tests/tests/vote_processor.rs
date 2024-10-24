use rsnano_core::{
    Amount, BlockBuilder, BlockEnum, KeyPair, Signature, StateBlock, Vote, VoteCode, VoteSource,
    WalletId, DEV_GENESIS_KEY,
};
use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_network::ChannelId;
use rsnano_node::{
    config::{FrontiersConfirmationMode, NodeFlags},
    consensus::{ActiveElectionsExt, ElectionBehavior, RepTier},
    stats::{DetailType, Direction, StatType},
    wallets::WalletsExt,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use test_helpers::{assert_timely, assert_timely_eq, setup_chain, start_election, System};

#[test]
fn codes() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    config.hinted_scheduler.enabled = false;
    config.optimistic_scheduler.enabled = false;
    let node = system.build_node().config(config).finish();
    let blocks = setup_chain(&node, 1, &DEV_GENESIS_KEY, false);
    let vote = Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN,
        0,
        vec![blocks[0].hash()],
    );
    let mut vote_invalid = vote.clone();
    vote_invalid.signature = Signature::new();

    let vote = Arc::new(vote);
    let vote_invalid = Arc::new(vote_invalid);
    let channel_id = ChannelId::from(42);

    // Invalid signature
    assert_eq!(
        VoteCode::Invalid,
        node.vote_processor
            .vote_blocking(&vote_invalid, channel_id, VoteSource::Live)
    );

    // No ongoing election (vote goes to vote cache)
    assert_eq!(
        VoteCode::Indeterminate,
        node.vote_processor
            .vote_blocking(&vote, channel_id, VoteSource::Live)
    );

    // Clear vote cache before starting election
    node.vote_cache.lock().unwrap().clear();

    // First vote from an account for an ongoing election
    start_election(&node, &blocks[0].hash());
    assert_timely(Duration::from_secs(5), || {
        node.active.election(&blocks[0].qualified_root()).is_some()
    });
    let _election = node.active.election(&blocks[0].qualified_root()).unwrap();
    assert_eq!(
        VoteCode::Vote,
        node.vote_processor
            .vote_blocking(&vote, channel_id, VoteSource::Live)
    );

    // Processing the same vote is a replay
    assert_eq!(
        VoteCode::Replay,
        node.vote_processor
            .vote_blocking(&vote, channel_id, VoteSource::Live)
    );

    // Invalid takes precedence
    assert_eq!(
        VoteCode::Invalid,
        node.vote_processor
            .vote_blocking(&vote_invalid, channel_id, VoteSource::Live)
    );

    // Once the election is removed (confirmed / dropped) the vote is again indeterminate
    assert!(node.active.erase(&blocks[0].qualified_root()));
    assert_eq!(
        VoteCode::Indeterminate,
        node.vote_processor
            .vote_blocking(&vote, channel_id, VoteSource::Live)
    );
}

#[test]
fn invalid_signature() {
    let mut system = System::new();
    let node = system.make_node();
    let chain = setup_chain(&node, 1, &DEV_GENESIS_KEY, false);
    let key = KeyPair::new();
    let vote = Vote::new(&key, Vote::TIMESTAMP_MIN, 0, vec![chain[0].hash()]);
    let mut vote_invalid = vote.clone();
    vote_invalid.signature = Signature::new();

    let vote = Arc::new(vote);
    let vote_invalid = Arc::new(vote_invalid);
    let election = start_election(&node, &chain[0].hash());
    assert_eq!(1, election.vote_count());
    let channel_id = ChannelId::from(42);

    node.vote_processor_queue
        .vote(vote_invalid, channel_id, VoteSource::Live);

    assert_timely_eq(Duration::from_secs(5), || election.vote_count(), 1);

    node.vote_processor_queue
        .vote(vote, channel_id, VoteSource::Live);

    assert_timely_eq(Duration::from_secs(5), || election.vote_count(), 2);
}

#[test]
fn overflow() {
    let mut system = System::new();
    let flags = NodeFlags {
        vote_processor_capacity: 1,
        ..Default::default()
    };
    let node = system.build_node().flags(flags).finish();
    let key = KeyPair::new();
    let vote = Arc::new(Vote::new(
        &key,
        Vote::TIMESTAMP_MIN,
        0,
        vec![*DEV_GENESIS_HASH],
    ));
    let start_time = Instant::now();
    // No way to lock the processor, but queueing votes in quick succession must result in overflow
    let mut not_processed = 0;
    const TOTAL: usize = 1000;
    for _ in 0..TOTAL {
        if !node
            .vote_processor_queue
            .vote(vote.clone(), ChannelId::from(42), VoteSource::Live)
        {
            not_processed += 1;
        }
    }

    assert!(not_processed > 0);
    assert!(not_processed < TOTAL);
    assert_eq!(
        not_processed as u64,
        node.stats
            .count(StatType::VoteProcessor, DetailType::Overfill, Direction::In)
    );

    // check that it did not timeout
    assert!(start_time.elapsed() < Duration::from_secs(10));
}

#[test]
fn weights() {
    let mut system = System::new();
    let node = system.make_node();

    // Create representatives of different weight levels
    // FIXME: Using `online_weight_minimum` because calculation of trended and online weight is broken when running tests
    let stake = node.config.online_weight_minimum;
    let level0 = stake / 5000; // 0.02%
    let level1 = stake / 500; // 0.2%
    let level2 = stake / 50; // 2%

    let key0 = KeyPair::new();
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();

    // Setup wallets and representatives
    let node1 = system.make_node();
    let node2 = system.make_node();
    let node3 = system.make_node();

    let nodes = vec![node.clone(), node1, node2, node3];

    let wallet0 = WalletId::random();
    nodes[0].wallets.create(wallet0);
    nodes[0]
        .wallets
        .insert_adhoc2(&wallet0, &DEV_GENESIS_KEY.private_key(), false)
        .unwrap();

    let wallet1 = WalletId::random();
    nodes[1].wallets.create(wallet1);
    nodes[1]
        .wallets
        .insert_adhoc2(&wallet1, &key0.private_key(), false)
        .unwrap();

    let wallet2 = WalletId::random();
    nodes[2].wallets.create(wallet2);
    nodes[2]
        .wallets
        .insert_adhoc2(&wallet2, &key1.private_key(), false)
        .unwrap();

    let wallet3 = WalletId::random();
    nodes[3].wallets.create(wallet3);
    nodes[3]
        .wallets
        .insert_adhoc2(&wallet3, &key2.private_key(), false)
        .unwrap();

    nodes[1]
        .wallets
        .set_representative(wallet1, key0.public_key(), false)
        .unwrap();
    nodes[2]
        .wallets
        .set_representative(wallet2, key1.public_key(), false)
        .unwrap();
    nodes[3]
        .wallets
        .set_representative(wallet3, key2.public_key(), false)
        .unwrap();

    // Send funds to set up different weight levels
    nodes[0]
        .wallets
        .send_action2(
            &wallet0,
            *DEV_GENESIS_ACCOUNT,
            key0.account(),
            level0,
            0,
            false,
            None,
        )
        .unwrap();
    nodes[0]
        .wallets
        .send_action2(
            &wallet0,
            *DEV_GENESIS_ACCOUNT,
            key1.account(),
            level1,
            0,
            false,
            None,
        )
        .unwrap();
    nodes[0]
        .wallets
        .send_action2(
            &wallet0,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            level2,
            0,
            false,
            None,
        )
        .unwrap();

    // Wait for representatives
    assert_timely_eq(
        Duration::from_secs(10),
        || node.online_reps.lock().unwrap().online_reps_count(),
        4,
    );

    // Wait for rep tiers to be updated
    node.stats.clear();
    assert_timely(Duration::from_secs(5), || {
        node.stats
            .count(StatType::RepTiers, DetailType::Updated, Direction::In)
            >= 2
    });

    assert_eq!(node.rep_tiers.tier(&key0.public_key()), RepTier::None);
    assert_eq!(node.rep_tiers.tier(&key1.public_key()), RepTier::Tier1);
    assert_eq!(node.rep_tiers.tier(&key2.public_key()), RepTier::Tier2);
    assert_eq!(
        node.rep_tiers.tier(&DEV_GENESIS_KEY.public_key()),
        RepTier::Tier3
    );
}

#[test]
fn empty_hashes() {
    let key = KeyPair::new();
    let vote = Arc::new(Vote::new(&key, Vote::TIMESTAMP_MIN, 0, vec![]));

    // In Rust, we don't need to explicitly test the creation of the vote object
    // as it would fail to compile if there were any issues.
    // However, we can add some assertions to verify the vote's properties:

    assert_eq!(vote.voting_account, key.public_key());
    assert_eq!(vote.timestamp, Vote::TIMESTAMP_MIN);
    assert_eq!(vote.hashes.len(), 0);
}

#[test]
fn timestamp_and_duration_masking() {
    let system = System::new();
    let key = KeyPair::new();
    let hash = vec![*DEV_GENESIS_HASH];
    let vote = Arc::new(Vote::new(&key, 0x123f, 0xf, hash));

    assert_eq!(vote.timestamp(), 0x1230);
    assert_eq!(vote.duration().as_millis(), 524288);
    assert_eq!(vote.duration_bits(), 0xf);
}

#[test]
fn no_broadcast_local_with_a_principal_representative() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    config.hinted_scheduler.enabled = false;
    config.optimistic_scheduler.enabled = false;
    let node = system.build_node().config(config).finish();

    // Reduce the weight of genesis to 2x default min voting weight
    let key = KeyPair::new();
    //let blocks = setup_chain(&node, 1, &DEV_GENESIS_KEY, false);
    //let send = blocks[0].clone();
    let new_balance = Amount::raw(node.config.online_weight_minimum.number() * 2);
    let send_amount = Amount::MAX - new_balance;

    let send = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        DEV_GENESIS_KEY.public_key(),
        new_balance,
        key.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    // Process the send block
    //node.process_active(send.clone());
    assert_eq!(
        node.process_local(send.clone()).unwrap(),
        BlockStatus::Progress
    );
    assert_eq!(
        new_balance,
        node.ledger.weight(&DEV_GENESIS_KEY.public_key())
    );

    // Insert account in wallet
    let wallet = WalletId::random();
    node.wallets.create(wallet);
    node.wallets
        .insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false)
        .unwrap();

    // Ensure that the node knows the genesis key in its wallet
    node.wallets.compute_reps();
    //assert!(node.wallets.rep_exists(&DEV_GENESIS_KEY.public_key()));
    //assert!(node.wallets.have_half_rep()); // Genesis balance after `send' is over both half_rep and PR threshold

    // Process a vote with a key that is in the local wallet
    let vote = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN,
        Vote::DURATION_MAX,
        vec![send.hash()],
    ));

    assert_eq!(
        VoteCode::Vote,
        node.vote_processor
            .vote_blocking(&vote, ChannelId::from(42), VoteSource::Live)
    );

    // Make sure the vote was processed
    let election = node.active.election(&send.qualified_root()).unwrap();
    let votes = election.mutex.lock().unwrap().last_votes.clone();
    assert!(votes.contains_key(&DEV_GENESIS_KEY.public_key()));
    assert_eq!(
        vote.timestamp,
        votes[&DEV_GENESIS_KEY.public_key()].timestamp
    );

    // Ensure the vote was not broadcast
    assert_eq!(
        0,
        node.stats
            .count(StatType::Message, DetailType::ConfirmAck, Direction::Out)
    );
    assert_eq!(
        1,
        node.stats
            .count(StatType::Message, DetailType::Publish, Direction::Out)
    );
}

#[test]
fn local_broadcast_without_a_representative() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.representative_vote_weight_minimum = Amount::zero();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    config.hinted_scheduler.enabled = false;
    config.optimistic_scheduler.enabled = false;
    let node = system.build_node().config(config).finish();

    // Reduce the weight of genesis to the minimum voting weight
    let key = KeyPair::new();
    let new_balance = node.config.vote_minimum;
    let send_amount = Amount::MAX - new_balance;

    // Build the send block
    let send = BlockBuilder::state()
        .account(DEV_GENESIS_KEY.public_key())
        .previous(*DEV_GENESIS_HASH)
        .representative(DEV_GENESIS_KEY.public_key())
        .balance(new_balance)
        .link(key.account())
        .sign(&DEV_GENESIS_KEY)
        .work(node.work_generate_dev((*DEV_GENESIS_HASH).into()))
        .build();

    // Process the send block
    assert_eq!(
        BlockStatus::Progress,
        node.process_local(send.clone()).unwrap()
    );
    //assert_timely(Duration::from_secs(10), || !node.active.is_empty());
    assert_eq!(
        new_balance,
        node.ledger.weight(&DEV_GENESIS_KEY.public_key())
    );

    // Start election for the send block
    node.active
        .insert(&Arc::new(send.clone()), ElectionBehavior::Manual, None);

    // Process a vote without a representative
    let vote = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN,
        Vote::DURATION_MAX,
        vec![send.hash()],
    ));

    assert_eq!(
        VoteCode::Vote,
        node.vote_processor
            .vote_blocking(&vote, ChannelId::from(42), VoteSource::Live)
    );

    // Make sure the vote was processed
    //let election = assert_timely(Duration::from_secs(5), || {
    //node.active.election(&send.qualified_root()).is_some()
    //});
    let election = node.active.election(&send.qualified_root()).unwrap();
    let votes = election.mutex.lock().unwrap().last_votes.clone();
    assert!(votes.contains_key(&DEV_GENESIS_KEY.public_key()));
    assert_eq!(
        vote.timestamp,
        votes[&DEV_GENESIS_KEY.public_key()].timestamp
    );

    // Ensure the vote was broadcast
    assert_eq!(
        1,
        node.stats
            .count(StatType::Message, DetailType::ConfirmAck, Direction::Out)
    );
    assert_eq!(
        1,
        node.stats
            .count(StatType::Message, DetailType::Publish, Direction::Out)
    );
}

#[test]
fn no_broadcast_local() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.representative_vote_weight_minimum = Amount::zero();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    config.hinted_scheduler.enabled = false;
    config.optimistic_scheduler.enabled = false;
    let node = system.build_node().config(config).finish();

    // Reduce the weight of genesis to 2x default min voting weight
    let key = KeyPair::new();
    let new_balance = Amount::raw(node.config.vote_minimum.number() * 2);
    let send_amount = Amount::MAX - new_balance;

    let send = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        DEV_GENESIS_KEY.public_key(),
        new_balance,
        key.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    // Process the send block
    assert_eq!(
        node.process_local(send.clone()).unwrap(),
        BlockStatus::Progress
    );
    //assert_timely(Duration::from_secs(10), || !node.active.is_empty());
    assert_eq!(
        new_balance,
        node.ledger.weight(&DEV_GENESIS_KEY.public_key())
    );

    // Insert account in wallet
    let wallet = WalletId::random();
    node.wallets.create(wallet);
    node.wallets
        .insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false)
        .unwrap();

    // Ensure that the node knows the genesis key in its wallet
    node.wallets.compute_reps();
    assert!(node.wallets.exists(&DEV_GENESIS_KEY.public_key()));
    //assert!(!node.wallets.have_half_rep()); // Genesis balance after `send' is less than the half_rep threshold

    // Process a vote with a key that is in the local wallet
    let vote = Arc::new(Vote::new(
        &DEV_GENESIS_KEY,
        Vote::TIMESTAMP_MIN,
        Vote::DURATION_MAX,
        vec![send.hash()],
    ));

    assert_eq!(
        VoteCode::Vote,
        node.vote_processor
            .vote_blocking(&vote, ChannelId::from(42), VoteSource::Live)
    );

    // Make sure the vote was processed
    let election = node.active.election(&send.qualified_root()).unwrap();
    let votes = election.mutex.lock().unwrap().last_votes.clone();
    assert!(votes.contains_key(&DEV_GENESIS_KEY.public_key()));
    assert_eq!(
        vote.timestamp,
        votes[&DEV_GENESIS_KEY.public_key()].timestamp
    );

    // Ensure the vote was not broadcast
    assert_eq!(
        0,
        node.stats
            .count(StatType::Message, DetailType::ConfirmAck, Direction::Out)
    );
    assert_eq!(
        1,
        node.stats
            .count(StatType::Message, DetailType::Publish, Direction::Out)
    );
}
