use rsnano_messages::{Message, TelemetryAck};
use rsnano_node::{
    config::NodeFlags,
    stats::{DetailType, Direction, StatType},
};
use std::{thread::sleep, time::Duration};

use super::helpers::{assert_never, assert_timely, make_fake_channel, System};

#[test]
fn invalid_signature() {
    let mut system = System::new();
    let node = system.make_node();

    let mut telemetry = node.telemetry.local_telemetry();
    telemetry.block_count = 9999; // Change data so signature is no longer valid
    let node_id = telemetry.node_id;
    let message = Message::TelemetryAck(TelemetryAck(Some(telemetry)));

    let channel = make_fake_channel(&node);
    channel.set_node_id(node_id);
    node.inbound_message_queue.put(message, channel);

    assert_timely(Duration::from_secs(5), || {
        node.stats.count(
            StatType::Telemetry,
            DetailType::InvalidSignature,
            Direction::In,
        ) > 0
    });
    assert_never(Duration::from_secs(1), || {
        node.stats
            .count(StatType::Telemetry, DetailType::Process, Direction::In)
            > 0
    });
}

#[test]
fn basic() {
    let mut system = System::new();
    let node_client = system.make_node();
    let node_server = system
        .build_node()
        .flags(NodeFlags {
            disable_ongoing_telemetry_requests: true,
            ..Default::default()
        })
        .finish();

    // Request telemetry metrics
    let channel = node_client
        .network
        .find_node_id(&node_server.get_node_id())
        .unwrap();

    assert_timely(Duration::from_secs(5), || {
        node_client
            .telemetry
            .get_telemetry(&channel.remote_addr())
            .is_some()
    });
    let telemetry_data = node_client
        .telemetry
        .get_telemetry(&channel.remote_addr())
        .unwrap();
    assert_eq!(node_server.get_node_id(), telemetry_data.node_id);

    // Check the metrics are correct
    // TODO

    // Call again straight away
    let telemetry_data2 = node_client
        .telemetry
        .get_telemetry(&channel.remote_addr())
        .unwrap();

    // Call again straight away
    let telemetry_data3 = node_client
        .telemetry
        .get_telemetry(&channel.remote_addr())
        .unwrap();

    // we expect at least one consecutive repeat of telemetry
    assert!(telemetry_data == telemetry_data2 || telemetry_data2 == telemetry_data3);

    // Wait the cache period and check cache is not used
    sleep(Duration::from_secs(3));

    let telemetry_data4 = node_client
        .telemetry
        .get_telemetry(&channel.remote_addr())
        .unwrap();

    assert_ne!(telemetry_data, telemetry_data4);
}
