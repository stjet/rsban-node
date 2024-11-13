use rsnano_core::Amount;
use rsnano_node::consensus::{Bucket, PriorityBucketConfig};
use test_helpers::System;

mod bucket {
    use super::*;
    use rsnano_core::BlockEnum;
    use std::sync::Arc;

    #[test]
    fn construction() {
        let mut system = System::new();
        let node = system.make_node();

        let bucket = Bucket::new(
            Amount::nano(1000),
            PriorityBucketConfig::default(),
            node.active.clone(),
            node.stats.clone(),
        );

        assert_eq!(bucket.can_accept(Amount::nano(999)), false);
        assert_eq!(bucket.can_accept(Amount::nano(1000)), true);
        assert_eq!(bucket.can_accept(Amount::nano(1001)), true);
        assert_eq!(bucket.len(), 0);
    }

    #[test]
    fn insert_one() {
        let mut system = System::new();
        let node = system.make_node();

        let bucket = Bucket::new(
            Amount::zero(),
            PriorityBucketConfig::default(),
            node.active.clone(),
            node.stats.clone(),
        );

        assert!(bucket.push(1000, Arc::new(BlockEnum::new_test_instance())));
        assert_eq!(bucket.len(), 1);
    }

    #[test]
    fn insert_duplicate() {
        let mut system = System::new();
        let node = system.make_node();

        let bucket = Bucket::new(
            Amount::zero(),
            PriorityBucketConfig::default(),
            node.active.clone(),
            node.stats.clone(),
        );

        let block = Arc::new(BlockEnum::new_test_instance());
        assert_eq!(bucket.push(1000, block.clone()), true);
        assert_eq!(bucket.push(1000, block), false);
    }

    #[test]
    fn insert_many() {
        let mut system = System::new();
        let node = system.make_node();

        let bucket = Bucket::new(
            Amount::zero(),
            PriorityBucketConfig::default(),
            node.active.clone(),
            node.stats.clone(),
        );

        let block0 = Arc::new(BlockEnum::new_test_instance_with_key(1));
        let block1 = Arc::new(BlockEnum::new_test_instance_with_key(2));
        let block2 = Arc::new(BlockEnum::new_test_instance_with_key(3));
        let block3 = Arc::new(BlockEnum::new_test_instance_with_key(3));
        assert!(bucket.push(2000, block0.clone()));
        assert!(bucket.push(1001, block1.clone()));
        assert!(bucket.push(1000, block2.clone()));
        assert!(bucket.push(900, block3.clone()));

        assert_eq!(bucket.len(), 4);
        let blocks = bucket.blocks();
        assert_eq!(blocks.len(), 4);
        // Ensure correct order
        assert_eq!(blocks[0], block3);
        assert_eq!(blocks[1], block2);
        assert_eq!(blocks[2], block1);
        assert_eq!(blocks[3], block0);
    }

    #[test]
    fn max_blocks() {
        let mut system = System::new();
        let node = system.make_node();

        let config = PriorityBucketConfig {
            max_blocks: 2,
            ..Default::default()
        };
        let bucket = Bucket::new(
            Amount::zero(),
            config,
            node.active.clone(),
            node.stats.clone(),
        );

        let block0 = Arc::new(BlockEnum::new_test_instance_with_key(1));
        let block1 = Arc::new(BlockEnum::new_test_instance_with_key(2));
        let block2 = Arc::new(BlockEnum::new_test_instance_with_key(3));
        let block3 = Arc::new(BlockEnum::new_test_instance_with_key(3));

        assert_eq!(bucket.push(2000, block0.clone()), true);
        assert_eq!(bucket.push(900, block1.clone()), true);
        assert_eq!(bucket.push(3000, block2.clone()), false);
        assert_eq!(bucket.push(1001, block3.clone()), true); // Evicts 2000
        assert_eq!(bucket.push(1000, block0.clone()), true); // Evicts 1001

        assert_eq!(bucket.len(), 2);
        let blocks = bucket.blocks();
        // Ensure correct order
        assert_eq!(blocks[0], block1);
        assert_eq!(blocks[1], block0);
    }
}

mod election_scheduler {
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use std::time::Duration;
    use test_helpers::{assert_timely, System};

    #[test]
    fn activate_one_timely() {
        let mut system = System::new();
        let node = system.make_node();

        let mut send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::nano(1000),
            (*DEV_GENESIS_ACCOUNT).into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.ledger
            .process(&mut node.ledger.rw_txn(), &mut send1)
            .unwrap();

        node.election_schedulers
            .priority
            .activate(&node.store.tx_begin_read(), &*DEV_GENESIS_ACCOUNT);

        assert_timely(Duration::from_secs(5), || {
            node.active.election(&send1.qualified_root()).is_some()
        });
    }

    #[test]
    fn activate_one_flush() {
        let mut system = System::new();
        let node = system.make_node();

        // Create a send block
        let mut send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::nano(1000),
            (*DEV_GENESIS_ACCOUNT).into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        // Process the block
        node.ledger
            .process(&mut node.store.tx_begin_write(), &mut send1)
            .unwrap();

        // Activate the account
        node.election_schedulers
            .priority
            .activate(&node.store.tx_begin_read(), &*DEV_GENESIS_ACCOUNT);

        // Assert that the election is created within 5 seconds
        assert_timely(Duration::from_secs(5), || {
            node.active.election(&send1.qualified_root()).is_some()
        });
    }
}
