use rsnano_core::{
    Account, Amount, BlockEnum, BlockHash, Epoch, KeyPair, PublicKey, SendBlock, StateBlock, Vote,
    VoteCode, VoteSource, DEV_GENESIS_KEY,
};
use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_network::ChannelId;
use rsnano_node::block_processing::BlockSource;
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely, assert_timely_eq, start_elections, System};

mod votes {
    use std::time::SystemTime;

    use rsnano_core::StateBlock;
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use rsnano_node::{
        config::{FrontiersConfirmationMode, NodeConfig},
        consensus::ActiveElectionsExt,
    };
    use test_helpers::start_election;

    use super::*;

    #[test]
    fn add_one() {
        let mut system = System::new();
        let node1 = system.make_node();
        let key1 = KeyPair::new();
        let send1 = BlockEnum::LegacySend(SendBlock::new(
            &DEV_GENESIS_HASH,
            &key1.public_key().as_account(),
            &(Amount::MAX - Amount::raw(100)),
            &DEV_GENESIS_KEY.private_key(),
            node1.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        node1.process(send1.clone()).unwrap();
        node1
            .election_schedulers
            .manual
            .push(send1.clone().into(), None);

        assert_timely(Duration::from_secs(5), || {
            node1.active.election(&send1.qualified_root()).is_some()
        });

        let election1 = node1.active.election(&send1.qualified_root()).unwrap();
        assert_eq!(election1.vote_count(), 1);
        let vote1 = Arc::new(Vote::new(
            &DEV_GENESIS_KEY,
            Vote::TIMESTAMP_MIN,
            0,
            vec![send1.hash()],
        ));
        assert_eq!(
            node1
                .vote_router
                .vote(&vote1, VoteSource::Live)
                .values()
                .next()
                .unwrap(),
            &VoteCode::Vote
        );
        let vote2 = Arc::new(Vote::new(
            &DEV_GENESIS_KEY,
            Vote::TIMESTAMP_MIN * 2,
            0,
            vec![send1.hash()],
        ));

        // Ignored due to vote cooldown
        assert_eq!(
            node1
                .vote_router
                .vote(&vote2, VoteSource::Live)
                .values()
                .next()
                .unwrap(),
            &VoteCode::Ignored
        );

        assert_eq!(election1.vote_count(), 2);
        assert_eq!(
            election1
                .mutex
                .lock()
                .unwrap()
                .last_votes
                .get(&DEV_GENESIS_PUB_KEY)
                .unwrap()
                .hash,
            send1.hash()
        );

        let guard = election1.mutex.lock().unwrap();
        let (hash, amount) = guard.last_tally.iter().next().unwrap();
        assert_eq!(*hash, send1.hash());
        assert_eq!(*amount, Amount::MAX - Amount::raw(100));
    }

    #[test]
    fn add_existing() {
        let mut system = System::new();
        let config = NodeConfig {
            online_weight_minimum: Amount::MAX,
            frontiers_confirmation: FrontiersConfirmationMode::Disabled,
            ..System::default_config()
        };
        let node1 = system.build_node().config(config).finish();
        let key1 = KeyPair::new();
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY, // No representative, blocks can't confirm
            Amount::MAX / 2 - Amount::nano(1000),
            key1.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        node1.process(send1.clone()).unwrap();
        let election1 = start_election(&node1, &send1.hash());
        let vote1 = Arc::new(Vote::new(
            &DEV_GENESIS_KEY,
            Vote::TIMESTAMP_MIN,
            0,
            vec![send1.hash()],
        ));
        node1.vote_router.vote(&vote1, VoteSource::Live);
        // Block is already processed from vote
        assert!(node1.active.publish_block(&send1.clone().into()));
        assert_eq!(
            election1
                .mutex
                .lock()
                .unwrap()
                .last_votes
                .get(&DEV_GENESIS_PUB_KEY)
                .unwrap()
                .timestamp,
            Vote::TIMESTAMP_MIN
        );
        let key2 = KeyPair::new();
        let send2 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY, // No representative, blocks can't confirm
            Amount::MAX / 2 - Amount::nano(1000),
            key2.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node1.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        assert_eq!(node1.active.publish_block(&send2.clone().into()), false);
        assert_timely(Duration::from_secs(5), || node1.active.active(&send2));
        let vote2 = Arc::new(Vote::new(
            &DEV_GENESIS_KEY,
            Vote::TIMESTAMP_MIN * 2,
            0,
            vec![send2.hash()],
        ));
        // Pretend we've waited the timeout
        election1
            .mutex
            .lock()
            .unwrap()
            .last_votes
            .get_mut(&DEV_GENESIS_PUB_KEY)
            .unwrap()
            .time = SystemTime::now() - Duration::from_secs(20);
        assert_eq!(
            node1
                .vote_router
                .vote(&vote2, VoteSource::Live)
                .get(&send2.hash())
                .unwrap(),
            &VoteCode::Vote
        );
        assert_eq!(
            election1
                .mutex
                .lock()
                .unwrap()
                .last_votes
                .get(&DEV_GENESIS_PUB_KEY)
                .unwrap()
                .timestamp,
            Vote::TIMESTAMP_MIN * 2
        );
        // Also resend the old vote, and see if we respect the timestamp
        election1
            .mutex
            .lock()
            .unwrap()
            .last_votes
            .get_mut(&DEV_GENESIS_PUB_KEY)
            .unwrap()
            .time = SystemTime::now() - Duration::from_secs(20);

        assert_eq!(
            node1
                .vote_router
                .vote(&vote1, VoteSource::Live)
                .get(&send1.hash())
                .unwrap(),
            &VoteCode::Replay
        );
        assert_eq!(
            election1
                .mutex
                .lock()
                .unwrap()
                .last_votes
                .get(&DEV_GENESIS_PUB_KEY)
                .unwrap()
                .timestamp,
            Vote::TIMESTAMP_MIN * 2
        );
        let votes = election1.mutex.lock().unwrap().last_votes.clone();
        assert_eq!(votes.len(), 2);
        assert!(votes.contains_key(&DEV_GENESIS_PUB_KEY));
        assert_eq!(votes.get(&DEV_GENESIS_PUB_KEY).unwrap().hash, send2.hash());
    }
}

#[test]
fn epoch_open_pending() {
    let mut system = System::new();
    let node1 = system.make_node();
    let key1 = KeyPair::new();
    let epoch_open = BlockEnum::State(StateBlock::new(
        key1.account(),
        BlockHash::zero(),
        PublicKey::zero(),
        Amount::zero(),
        node1.ledger.epoch_link(Epoch::Epoch1).unwrap(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(&key1),
    ));
    let status = node1.process(epoch_open.clone()).unwrap_err();
    assert_eq!(status, BlockStatus::GapEpochOpenPending);
    node1.block_processor.add(
        epoch_open.clone().into(),
        BlockSource::Live,
        ChannelId::LOOPBACK,
    );
    // Waits for the block to get saved in the database
    assert_timely_eq(Duration::from_secs(10), || node1.unchecked.len(), 1);
    // Open block should be inserted into unchecked
    let blocks = node1.unchecked.get(&epoch_open.account().into());
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0].block.as_ref().unwrap().full_hash(),
        epoch_open.full_hash()
    );
    // New block to process epoch open
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(100),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));
    node1
        .block_processor
        .add(send1.into(), BlockSource::Live, ChannelId::LOOPBACK);
    assert_timely(Duration::from_secs(10), || {
        node1.block_exists(&epoch_open.hash())
    });
}

#[test]
fn block_hash_account_conflict() {
    let mut system = System::new();
    let node1 = system.make_node();
    let key1 = KeyPair::new();
    let key2 = KeyPair::new();

    /*
     * Generate a send block whose destination is a block hash already
     * in the ledger and not an account
     */
    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::raw(100),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(*DEV_GENESIS_HASH),
    ));

    let receive1 = BlockEnum::State(StateBlock::new(
        key1.account(),
        BlockHash::zero(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(100),
        send1.hash().into(),
        &key1,
        node1.work_generate_dev(&key1),
    ));

    /*
     * Note that the below link is a block hash when this is intended
     * to represent a send state block. This can generally never be
     * received , except by epoch blocks, which can sign an open block
     * for arbitrary accounts.
     */
    let send2 = BlockEnum::State(StateBlock::new(
        key1.account(),
        receive1.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::raw(90),
        receive1.hash().into(),
        &key1,
        node1.work_generate_dev(receive1.hash()),
    ));

    /*
     * Generate an epoch open for the account with the same value as the block hash
     */
    let open_epoch1 = BlockEnum::State(StateBlock::new(
        Account::from_bytes(*receive1.hash().as_bytes()),
        BlockHash::zero(),
        PublicKey::zero(),
        Amount::zero(),
        node1.ledger.epoch_link(Epoch::Epoch1).unwrap(),
        &DEV_GENESIS_KEY,
        node1.work_generate_dev(receive1.hash()),
    ));

    node1.process_multi(&[
        send1.clone(),
        receive1.clone(),
        send2.clone(),
        open_epoch1.clone(),
    ]);

    start_elections(
        &node1,
        &[
            send1.hash(),
            receive1.hash(),
            send2.hash(),
            open_epoch1.hash(),
        ],
        false,
    );
    let election1 = node1.active.election(&send1.qualified_root()).unwrap();
    let election2 = node1.active.election(&receive1.qualified_root()).unwrap();
    let election3 = node1.active.election(&send2.qualified_root()).unwrap();
    let election4 = node1
        .active
        .election(&open_epoch1.qualified_root())
        .unwrap();

    assert_eq!(election1.winner_hash().unwrap(), send1.hash());
    assert_eq!(election2.winner_hash().unwrap(), receive1.hash());
    assert_eq!(election3.winner_hash().unwrap(), send2.hash());
    assert_eq!(election4.winner_hash().unwrap(), open_epoch1.hash());
}
