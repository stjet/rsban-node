use rsnano_core::{KeyPair, Signature, Vote, VoteCode, VoteSource, WalletId, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_network::ChannelId;
use rsnano_node::{
    config::{FrontiersConfirmationMode, NodeFlags}, consensus::RepTier, stats::{DetailType, Direction, StatType}, wallets::WalletsExt
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
    let node0 = system.make_node();
    let node1 = system.make_node();
    let node2 = system.make_node();
    let node3 = system.make_node();
    
    let nodes = vec![node0, node1, node2, node3];
    
    let wallet0= WalletId::random(); 
    nodes[0].wallets.create(wallet0);
    nodes[0].wallets.insert_adhoc2(&wallet0, &DEV_GENESIS_KEY.private_key(), false).unwrap();
    
    let wallet1= WalletId::random(); 
    nodes[1].wallets.create(wallet1);
    nodes[1].wallets.insert_adhoc2(&wallet1, &key0.private_key(), false).unwrap();
    
    let wallet2= WalletId::random(); 
    nodes[2].wallets.create(wallet2);
    nodes[2].wallets.insert_adhoc2(&wallet2, &key1.private_key(), false).unwrap();
    
    let wallet3= WalletId::random(); 
    nodes[3].wallets.create(wallet3);
    nodes[3].wallets.insert_adhoc2(&wallet3, &key2.private_key(), false).unwrap();
    
    nodes[1].wallets.set_representative(wallet1, key0.public_key(), false).unwrap();
    nodes[2].wallets.set_representative(wallet2, key1.public_key(), false).unwrap();
    nodes[3].wallets.set_representative(wallet3, key2.public_key(), false).unwrap();

    // Send funds to set up different weight levels
    nodes[0].wallets.send_action2(&wallet0, *DEV_GENESIS_ACCOUNT, key0.account(), level0, 0, false, None).unwrap();
    nodes[0].wallets.send_action2(&wallet0, *DEV_GENESIS_ACCOUNT, key1.account(), level1, 0, false, None).unwrap();
    nodes[0].wallets.send_action2(&wallet0, *DEV_GENESIS_ACCOUNT, key2.account(), level2, 0, false, None).unwrap();

    // Wait for representatives
    assert_timely_eq(Duration::from_secs(10), || node.online_reps.lock().unwrap().online_reps_count(), 4);

    // Wait for rep tiers to be updated
    node.stats.clear();
    assert_timely(Duration::from_secs(5), || {
        node.stats.count(StatType::RepTiers, DetailType::Updated, Direction::In) >= 2
    });

    assert_eq!(node.rep_tiers.tier(&key0.public_key()), RepTier::None);
    assert_eq!(node.rep_tiers.tier(&key1.public_key()), RepTier::Tier1);
    assert_eq!(node.rep_tiers.tier(&key2.public_key()), RepTier::Tier2);
    assert_eq!(node.rep_tiers.tier(&DEV_GENESIS_KEY.public_key()), RepTier::Tier3);
}