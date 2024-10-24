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
    use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{
        config::{FrontiersConfirmationMode, NodeConfig},
        consensus::{ActiveElectionsConfig, ActiveElectionsExt},
        wallets::WalletsExt,
    };
    use std::time::Duration;
    use test_helpers::{assert_timely, assert_timely_eq, System};

    #[test]
    fn activate_one_timely() {
        let mut system = System::new();
        let config = NodeConfig {
            frontiers_confirmation: FrontiersConfirmationMode::Disabled,
            ..System::default_config()
        };
        let node = system.build_node().config(config).finish();
        let wallet_id = node.wallets.wallet_ids()[0];
        node.wallets
            .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), true)
            .unwrap();

        // Create a send block
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::raw(1_000_000_000_000_000_000_000_000),
            (*DEV_GENESIS_ACCOUNT).into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        // Process the block
        node.process_active(send1.clone());
        assert_timely(Duration::from_secs(5), || {
            node.block(&send1.hash()).is_some()
        });

        // Activate the account
        node.election_schedulers
            .priority
            .activate(&node.store.tx_begin_read(), &*DEV_GENESIS_ACCOUNT);

        // Assert that the election is created within 5 seconds
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
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::raw(1_000_000_000_000_000_000_000_000),
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

    #[test]
    fn no_vacancy() {
        let mut system = System::new();
        let mut active_elections = ActiveElectionsConfig::default();
        active_elections.size = 1;
        let config = NodeConfig {
            active_elections,
            frontiers_confirmation: FrontiersConfirmationMode::Disabled,
            ..System::default_config()
        };
        let node = system.build_node().config(config).finish();

        let key = KeyPair::new();

        // Prepare 2 accounts
        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::raw(1_000_000_000_000_000_000_000_000),
            key.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));
        assert_eq!(
            node.process_local(send.clone()).unwrap(),
            BlockStatus::Progress
        );
        //node.process_confirmed(ElectionStatus::new(send.clone()));

        let receive = BlockEnum::State(StateBlock::new(
            key.account(),
            BlockHash::zero(),
            key.public_key(),
            Amount::raw(1_000_000_000_000_000_000_000_000),
            send.hash().into(),
            &key,
            node.work_generate_dev(key.public_key().into()),
        ));
        assert_eq!(
            node.process_local(receive.clone()).unwrap(),
            BlockStatus::Progress
        );
        //node.process_confirmed(ElectionStatus::new(receive.clone()));
        node.confirm(receive.hash());

        assert_timely(Duration::from_secs(5), || {
            node.confirm_multi(&[send.clone(), receive.clone()]);
            true
        });

        // Process two eligible transactions
        let block1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            send.hash(),
            *DEV_GENESIS_PUB_KEY,
            node.balance(&*DEV_GENESIS_ACCOUNT) - Amount::raw(2_000_000_000_000_000_000_000_000),
            (*DEV_GENESIS_ACCOUNT).into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(send.hash().into()),
        ));
        assert_eq!(
            node.process_local(block1.clone()).unwrap(),
            BlockStatus::Progress
        );

        // There is vacancy so it should be inserted
        node.election_schedulers
            .priority
            .activate(&node.store.tx_begin_read(), &*DEV_GENESIS_ACCOUNT);
        assert_timely(Duration::from_secs(5), || {
            node.active.election(&block1.qualified_root()).is_some()
        });

        let block2 = BlockEnum::State(StateBlock::new(
            key.account(),
            receive.hash(),
            key.public_key(),
            Amount::zero(),
            key.account().into(),
            &key,
            node.work_generate_dev(receive.hash().into()),
        ));
        assert_eq!(
            node.process_local(block2.clone()).unwrap(),
            BlockStatus::Progress
        );

        // There is no vacancy so it should stay queued
        node.election_schedulers
            .priority
            .activate(&node.store.tx_begin_read(), &key.account());
        assert_timely_eq(
            Duration::from_secs(5),
            || node.election_schedulers.priority.len(),
            1,
        );
        assert!(node.active.election(&block2.qualified_root()).is_none());

        // Election confirmed, next in queue should begin
        let election = node.active.election(&block1.qualified_root()).unwrap();
        node.active.force_confirm(&election);
        assert_timely(Duration::from_secs(5), || {
            node.active.election(&block2.qualified_root()).is_some()
        });
        assert!(node.election_schedulers.priority.len() == 0);
    }
}
