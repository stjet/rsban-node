use crate::tests::helpers::{assert_never, assert_timely, assert_timely_eq, System};
use rsnano_core::{work::WorkPool, Amount, BlockEnum, KeyPair, SendBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_node::{
    node::NodeExt,
    stats::{DetailType, Direction, StatType},
    transport::PeerConnectorExt,
};
use std::time::Duration;
use tracing::error;

#[test]
fn local_block_broadcast() {
    let mut system = System::new();

    let mut node_config = System::default_config();
    node_config.priority_scheduler_enabled = false;
    node_config.hinted_scheduler.enabled = false;
    node_config.optimistic_scheduler.enabled = false;
    node_config.local_block_broadcaster.rebroadcast_interval = Duration::from_secs(1);

    let node1 = system.build_node().config(node_config).finish();
    let node2 = system.make_disconnected_node();

    let key1 = KeyPair::new();
    let latest_hash = *DEV_GENESIS_HASH;

    let send1 = BlockEnum::LegacySend(SendBlock::new(
        &latest_hash,
        &key1.public_key(),
        &(Amount::MAX - Amount::nano(1000)),
        &DEV_GENESIS_KEY.private_key(),
        &DEV_GENESIS_KEY.public_key(),
        system.work.generate_dev2(latest_hash.into()).unwrap(),
    ));

    let qualified_root = send1.qualified_root();
    let send_hash = send1.hash();
    node1.process_local(send1).unwrap();

    assert_never(Duration::from_millis(500), || {
        node1.active.active_root(&qualified_root)
    });

    // Wait until a broadcast is attempted
    assert_timely_eq(
        Duration::from_secs(5),
        || node1.local_block_broadcaster.len(),
        1,
    );
    assert_timely(
        Duration::from_secs(5),
        || {
            node1.stats.count(
                StatType::LocalBlockBroadcaster,
                DetailType::Broadcast,
                Direction::Out,
            ) >= 1
        },
        "no broadcast sent",
    );

    // The other node should not have received a block
    assert_never(Duration::from_millis(500), || {
        node2.block(&send_hash).is_some()
    });

    error!(
        "node2 local addr is {:?}",
        node2.tcp_listener.local_address()
    );

    // Connect the nodes and check that the block is propagated
    node1
        .peer_connector
        .connect_to(node2.tcp_listener.local_address());
    assert_timely(
        Duration::from_secs(5),
        || node1.network.find_node_id(&node2.get_node_id()).is_some(),
        "node2 not connected",
    );
    assert_timely(
        Duration::from_secs(10),
        || node2.block(&send_hash).is_some(),
        "block not received",
    )
}

#[test]
fn online_reps() {
    let mut system = System::new();
    let node = system.make_node();
    // 1 sample of minimum weight
    assert_eq!(
        node.online_reps.lock().unwrap().trended_weight(),
        node.config.online_weight_minimum
    );
    assert_eq!(
        node.online_reps.lock().unwrap().online_weight(),
        Amount::zero()
    );

    node.online_reps
        .lock()
        .unwrap()
        .vote_observed(*DEV_GENESIS_ACCOUNT);

    assert_eq!(
        node.online_reps.lock().unwrap().online_weight(),
        Amount::MAX
    );
    // 1 minimum, 1 maximum
    assert_eq!(
        node.online_reps.lock().unwrap().trended_weight(),
        node.config.online_weight_minimum
    );

    node.ongoing_online_weight_calculation();
    assert_eq!(
        node.online_reps.lock().unwrap().trended_weight(),
        Amount::MAX
    );
}
