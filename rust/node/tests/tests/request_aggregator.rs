use std::{sync::Arc, time::Duration};

use super::helpers::{assert_timely, assert_timely_eq, System};
use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_node::{
    config::FrontiersConfirmationMode,
    stats::{DetailType, Direction, StatType},
    transport::ChannelEnum,
    wallets::WalletsExt,
};

#[test]
fn one() {
    let mut system = System::new();
    let mut config = System::default_config();
    config.frontiers_confirmation = FrontiersConfirmationMode::Disabled;
    let node = system.build_node().config(config).finish();
    node.wallets
        .insert_adhoc2(
            &node.wallets.wallet_ids()[0],
            &DEV_GENESIS_KEY.private_key(),
            true,
        )
        .unwrap();

    let mut send1 = BlockEnum::State(StateBlock::new(
        *DEV_GENESIS_ACCOUNT,
        *DEV_GENESIS_HASH,
        *DEV_GENESIS_ACCOUNT,
        Amount::MAX - Amount::nano(1000),
        (*DEV_GENESIS_ACCOUNT).into(),
        &DEV_GENESIS_KEY,
        node.work_generate_dev((*DEV_GENESIS_HASH).into()),
    ));

    let request = vec![(send1.hash(), send1.root())];

    // Not yet in the ledger
    let dummy_channel = Arc::new(ChannelEnum::new_null());
    node.request_aggregator
        .request(request.clone(), dummy_channel.clone());
    assert_timely(
        Duration::from_secs(3),
        || node.request_aggregator.is_empty(),
        "aggregator not empty",
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Requests,
                DetailType::RequestsUnknown,
                Direction::In,
            )
        },
        1,
    );

    // Process and confirm
    node.ledger
        .process(&mut node.ledger.rw_txn(), &mut send1)
        .unwrap();
    node.confirm(send1.hash());

    // In the ledger but no vote generated yet
    node.request_aggregator
        .request(request.clone(), dummy_channel.clone());
    assert_timely(
        Duration::from_secs(3),
        || node.request_aggregator.is_empty(),
        "aggregator not empty",
    );
    assert_timely(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Requests,
                DetailType::RequestsGeneratedVotes,
                Direction::In,
            ) > 0
        },
        "no votes generated",
    );

    // Already cached
    // TODO: This is outdated, aggregator should not be using cache
    node.request_aggregator.request(request, dummy_channel);
    assert_timely(
        Duration::from_secs(3),
        || node.request_aggregator.is_empty(),
        "aggregator not empty",
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Aggregator,
                DetailType::AggregatorAccepted,
                Direction::In,
            )
        },
        3,
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Aggregator,
                DetailType::AggregatorDropped,
                Direction::In,
            )
        },
        0,
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Requests,
                DetailType::RequestsUnknown,
                Direction::In,
            )
        },
        1,
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Requests,
                DetailType::RequestsGeneratedVotes,
                Direction::In,
            )
        },
        2,
    );
    assert_timely_eq(
        Duration::from_secs(3),
        || {
            node.stats.count(
                StatType::Requests,
                DetailType::RequestsCannotVote,
                Direction::In,
            )
        },
        0,
    );
}
