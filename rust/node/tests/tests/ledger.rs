use rsnano_core::{
    Amount, BlockEnum, KeyPair, SendBlock, Vote, VoteCode, VoteSource, DEV_GENESIS_KEY,
};
use rsnano_ledger::{DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use std::{sync::Arc, time::Duration};
use test_helpers::{assert_timely, System};

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
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
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
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
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
            node1.work_generate_dev((*DEV_GENESIS_HASH).into()),
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
