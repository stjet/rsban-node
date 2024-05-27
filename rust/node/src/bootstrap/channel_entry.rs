use crate::{
    transport::{Channel, ChannelEnum, ChannelTcp, Socket, SocketExtensions, TcpServer},
    utils::{ipv4_address_or_ipv6_subnet, map_address_to_subnetwork},
};
use rsnano_core::Account;
use std::{
    net::{Ipv6Addr, SocketAddrV6},
    sync::Arc,
    time::SystemTime,
};

pub struct ChannelEntry {
    pub channel: Arc<ChannelEnum>,
    pub response_server: Option<Arc<TcpServer>>,
}

impl ChannelEntry {
    pub fn new(channel: Arc<ChannelEnum>, response_server: Option<Arc<TcpServer>>) -> Self {
        Self {
            channel,
            response_server,
        }
    }

    pub fn tcp_channel(&self) -> &Arc<ChannelTcp> {
        match self.channel.as_ref() {
            ChannelEnum::Tcp(tcp) => tcp,
            _ => panic!("not a tcp channel"),
        }
    }

    pub fn endpoint(&self) -> SocketAddrV6 {
        self.channel.remote_endpoint()
    }

    pub fn last_packet_sent(&self) -> SystemTime {
        self.channel.get_last_packet_sent()
    }

    pub fn last_bootstrap_attempt(&self) -> SystemTime {
        self.channel.get_last_bootstrap_attempt()
    }

    pub fn close_socket(&self) {
        if let ChannelEnum::Tcp(tcp) = self.channel.as_ref() {
            tcp.socket.close();
        }
    }

    pub fn node_id(&self) -> Option<Account> {
        self.channel.get_node_id()
    }

    pub fn network_version(&self) -> u8 {
        self.channel.network_version()
    }

    pub fn ip_address(&self) -> Ipv6Addr {
        ipv4_address_or_ipv6_subnet(self.endpoint().ip())
    }

    pub fn subnetwork(&self) -> Ipv6Addr {
        map_address_to_subnetwork(self.endpoint().ip())
    }
}
