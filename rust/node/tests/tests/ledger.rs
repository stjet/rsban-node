use rsnano_core::{Amount, BlockEnum, KeyPair, SendBlock, DEV_GENESIS_KEY};
use rsnano_ledger::DEV_GENESIS_HASH;
use std::time::Duration;
use test_helpers::{assert_timely, System};

mod votes {
    use std::sync::Arc;

    use rsnano_core::{Vote, VoteCode, VoteSource};
    use rsnano_ledger::DEV_GENESIS_PUB_KEY;

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
}
