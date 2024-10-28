use crate::command_handler::RpcCommandHandler;
use anyhow::anyhow;
use rsnano_messages::{Keepalive, Message};
use rsnano_network::{DropPolicy, TrafficType};
use rsnano_rpc_messages::{AddressWithPortArgs, StartedDto};
use std::net::SocketAddrV6;

impl RpcCommandHandler {
    pub(crate) fn keepalive(&self, args: AddressWithPortArgs) -> anyhow::Result<StartedDto> {
        let peering_addr = SocketAddrV6::new(args.address.into(), args.port, 0, 0);
        let id = self
            .node
            .network_info
            .read()
            .unwrap()
            .find_realtime_channel_by_peering_addr(&peering_addr)
            .ok_or_else(|| anyhow!(Self::PEER_NOT_FOUND))?;

        let keepalive = Message::Keepalive(Keepalive::default());
        let mut publisher = self.node.message_publisher.lock().unwrap();

        publisher.try_send(
            id,
            &keepalive,
            DropPolicy::ShouldNotDrop,
            TrafficType::Generic,
        );

        Ok(StartedDto::new(true))
    }
}
