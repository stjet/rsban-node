use std::time::Duration;

use rsnano_core::{Amount, BlockEnum, KeyPair, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_PUB_KEY};
use rsnano_node::{
    config::{FrontiersConfirmationMode, NodeConfig},
    stats::{DetailType, Direction, StatType},
};
use test_helpers::{assert_timely_eq, System};

#[test]
fn observer_callbacks() {
    let mut system = System::new();
    let config = NodeConfig {
        frontiers_confirmation: FrontiersConfirmationMode::Disabled,
        ..System::default_config()
    };
    let node = system.build_node().config(config).finish();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let latest = node.latest(&DEV_GENESIS_ACCOUNT);

    let key1 = KeyPair::new();
    let send = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        latest,
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::nano(1000),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(latest.into()),
    ));

    let send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        send.hash(),
        *DEV_GENESIS_PUB_KEY,
        Amount::MAX - Amount::nano(2000),
        key1.account().into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev(send.hash().into()),
    ));

    node.process_multi(&[send.clone(), send1.clone()]);

    node.confirming_set.add(send1.hash());

    // Callback is performed for all blocks that are confirmed
    assert_timely_eq(
        Duration::from_secs(5),
        || {
            node.stats
                .count_all(StatType::ConfirmationObserver, Direction::Out)
        },
        2,
    );

    assert_eq!(
        node.stats.count(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In
        ),
        2
    );
    assert_eq!(node.ledger.cemented_count(), 3);
    assert_eq!(node.active.vote_applier.election_winner_details_len(), 0);
}
