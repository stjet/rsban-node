use rsnano_core::{Amount, Block, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::config::{FrontiersConfirmationMode, NodeConfig};
use std::{thread::sleep, time::Duration};
use test_helpers::{assert_timely_eq, System};

#[test]
fn frontiers_confirmation_mode() {
    let key = KeyPair::new();
    // Always mode
    {
        let mut system = System::new();
        let config = NodeConfig {
            frontiers_confirmation: FrontiersConfirmationMode::Always,
            ..System::default_config()
        };
        let node = system.build_node().config(config).finish();
        let send = Block::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::nano(1000),
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        node.process(send).unwrap();
        assert_timely_eq(Duration::from_secs(5), || node.active.len(), 1);
    }
    // Auto mode
    {
        let mut system = System::new();
        let config = NodeConfig {
            frontiers_confirmation: FrontiersConfirmationMode::Automatic,
            ..System::default_config()
        };
        let node = system.build_node().config(config).finish();
        let send = Block::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::nano(1000),
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        node.process(send).unwrap();
        assert_timely_eq(Duration::from_secs(5), || node.active.len(), 1);
    }
    // Disabled mode
    {
        let mut system = System::new();
        let config = NodeConfig {
            frontiers_confirmation: FrontiersConfirmationMode::Disabled,
            ..System::default_config()
        };
        let node = system.build_node().config(config).finish();
        let send = Block::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::nano(1000),
            key.public_key().as_account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(*DEV_GENESIS_HASH),
        ));
        node.process(send).unwrap();
        node.insert_into_wallet(&DEV_GENESIS_KEY);
        sleep(Duration::from_secs(1));
        assert_eq!(0, node.active.len());
    }
}
