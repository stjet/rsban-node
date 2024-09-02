use rsnano_messages::{Keepalive, Message};
use rsnano_network::{DropPolicy, TrafficType};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;
use std::{
    net::{Ipv6Addr, SocketAddrV6},
    sync::Arc,
};

pub async fn keepalive(
    node: Arc<Node>,
    enable_control: bool,
    address: Ipv6Addr,
    port: u16,
) -> String {
    if enable_control {
        let peer = SocketAddrV6::new(address.into(), port, 0, 0);

        let channel = node
            .network_info
            .read()
            .unwrap()
            .find_realtime_channel_by_remote_addr(&peer)
            .unwrap()
            .clone();

        node.peer_connector.connect_to(peer);

        let keepalive = Message::Keepalive(Keepalive::default());
        let mut publisher = node.message_publisher.lock().unwrap();
        publisher.try_send(
            channel.channel_id(),
            &keepalive,
            DropPolicy::ShouldNotDrop,
            TrafficType::Generic,
        );
        to_string_pretty(&SuccessDto::new()).unwrap()
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_node::stats::{DetailType, Direction, StatType};
    use std::{net::Ipv6Addr, time::Duration};
    use test_helpers::{assert_timely_msg, establish_tcp, System};

    #[test]
    fn keep_alive_rpc_command() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let node_peer = system.make_node();
        let channel = establish_tcp(&node_peer, &node);

        let timestamp_before_keepalive = channel.last_activity();
        let keepalive_count =
            node.stats
                .count(StatType::Message, DetailType::Keepalive, Direction::In);

        node.tokio.block_on(async {
            rpc_client
                .keepalive(Ipv6Addr::LOCALHOST, 7676)
                .await
                .unwrap();
        });

        assert_timely_msg(
            Duration::from_secs(3),
            || {
                node.stats
                    .count(StatType::Message, DetailType::Keepalive, Direction::In)
                    >= keepalive_count + 1
            },
            "keepalive count",
        );

        let timestamp_after_keepalive = channel.last_activity();
        assert!(timestamp_after_keepalive > timestamp_before_keepalive);

        server.abort();
    }
}
