use std::{net::SocketAddr, sync::Arc, time::SystemTime};

use rsnano_core::Account;

use crate::transport::{ChannelEnum, ChannelTcp, SocketImpl, TcpServer};

pub struct ChannelTcpWrapper {
    pub channel: Arc<ChannelEnum>,
    socket: Arc<SocketImpl>,
    pub response_server: Option<Arc<TcpServer>>,
}

impl ChannelTcpWrapper {
    pub fn new(
        channel: Arc<ChannelEnum>,
        socket: Arc<SocketImpl>,
        response_server: Option<Arc<TcpServer>>,
    ) -> Self {
        Self {
            channel,
            socket,
            response_server,
        }
    }

    fn tcp_channel(&self) -> &ChannelTcp {
        match self.channel.as_ref() {
            ChannelEnum::Tcp(tcp) => tcp,
            _ => panic!("not a tcp channel"),
        }
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.tcp_channel().endpoint()
    }

    pub fn last_packet_sent(&self) -> SystemTime {
        self.channel.as_channel().get_last_packet_sent()
    }

    pub fn last_bootstrap_attempt(&self) -> SystemTime {
        self.channel.as_channel().get_last_bootstrap_attempt()
    }

    pub fn socket(&self) -> Option<Arc<SocketImpl>> {
        self.tcp_channel().socket()
    }

    pub fn node_id(&self) -> Option<Account> {
        self.channel.as_channel().get_node_id()
    }

    pub fn network_version(&self) -> u8 {
        self.tcp_channel().network_version()
    }
}
