use rsnano_messages::{Keepalive, Message};
use rsnano_network::{DropPolicy, TrafficType};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, StartedDto};
use std::{
    net::{Ipv6Addr, SocketAddrV6},
    sync::Arc,
};

pub async fn keepalive(
    node: Arc<Node>,
    enable_control: bool,
    address: Ipv6Addr,
    port: u16,
) -> RpcDto {
    if !enable_control {
        return RpcDto::Error(ErrorDto::RPCControlDisabled)
    }

    let peering_addr = SocketAddrV6::new(address.into(), port, 0, 0);
    let channel_id = node
        .network_info
        .read()
        .unwrap()
        .find_realtime_channel_by_peering_addr(&peering_addr);

    match channel_id {
        Some(id) => {
            let keepalive = Message::Keepalive(Keepalive::default());
            let mut publisher = node.message_publisher.lock().unwrap();

            publisher.try_send(
                id,
                &keepalive,
                DropPolicy::ShouldNotDrop,
                TrafficType::Generic,
            );

            RpcDto::Keepalive(StartedDto::new(true))
        }
        None => RpcDto::Error(ErrorDto::PeerNotFound)
    }
}
