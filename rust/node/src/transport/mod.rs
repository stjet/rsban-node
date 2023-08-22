mod bandwidth_limiter;
mod channel_fake;
mod channel_inproc;
mod channel_tcp;
mod message_deserializer;
mod network_filter;
mod peer_exclusion;
mod socket;
mod syn_cookies;
mod tcp_channels;
mod tcp_message_manager;
mod tcp_server;
mod tcp_server_factory;
mod token_bucket;
mod write_queue;

use std::time::SystemTime;

pub use bandwidth_limiter::{
    BandwidthLimitType, BandwidthLimiter, OutboundBandwidthLimiter, OutboundBandwidthLimiterConfig,
};
pub use channel_fake::ChannelFake;
pub use channel_inproc::ChannelInProc;
pub use channel_tcp::{ChannelTcp, ChannelTcpObserver, IChannelTcpObserverWeakPtr, TcpChannelData};
pub use message_deserializer::{
    MessageDeserializer, MessageDeserializerExt, ParseStatus, ReadQuery,
};
pub use network_filter::NetworkFilter;
pub use peer_exclusion::PeerExclusion;
use rsnano_core::Account;
pub use socket::*;
pub use syn_cookies::{Cookie, SynCookies};
pub use tcp_channels::{TcpChannels, TcpChannelsImpl, TcpChannelsOptions, TcpEndpointAttempt};
pub use tcp_message_manager::{TcpMessageItem, TcpMessageManager};
pub use tcp_server::{
    BootstrapMessageVisitor, HandshakeMessageVisitor, HandshakeMessageVisitorImpl,
    NullTcpServerObserver, RealtimeMessageVisitor, RealtimeMessageVisitorImpl, TcpServer,
    TcpServerExt, TcpServerObserver,
};
pub use tcp_server_factory::TcpServerFactory;
use token_bucket::TokenBucket;
pub use write_queue::WriteCallback;

#[repr(u8)]
#[derive(FromPrimitive)]
pub enum TransportType {
    Undefined = 0,
    Tcp = 1,
    Loopback = 2,
    Fake = 3,
}

pub trait Channel {
    fn channel_id(&self) -> usize;
    fn is_temporary(&self) -> bool;
    fn set_temporary(&self, temporary: bool);
    fn get_last_bootstrap_attempt(&self) -> SystemTime; //todo switch back to Instant
    fn set_last_bootstrap_attempt(&self, time: SystemTime); //todo switch back to Instant
    fn get_last_packet_received(&self) -> SystemTime; //todo switch back to Instant
    fn set_last_packet_received(&self, instant: SystemTime); //todo switch back to Instant
    fn get_last_packet_sent(&self) -> SystemTime; //todo switch back to Instant
    fn set_last_packet_sent(&self, instant: SystemTime); //todo switch back to Instant
    fn get_node_id(&self) -> Option<Account>;
    fn set_node_id(&self, id: Account);
    fn is_alive(&self) -> bool;
    fn get_type(&self) -> TransportType;
}

#[derive(FromPrimitive, Copy, Clone)]
pub enum TrafficType {
    Generic,
    /** For bootstrap (asc_pull_ack, asc_pull_req) traffic */
    Bootstrap,
}

pub enum ChannelEnum {
    Tcp(ChannelTcp),
    InProc(ChannelInProc),
    Fake(ChannelFake),
}

impl ChannelEnum {
    pub fn as_channel(&self) -> &dyn Channel {
        match &self {
            ChannelEnum::Tcp(tcp) => tcp,
            ChannelEnum::InProc(inproc) => inproc,
            ChannelEnum::Fake(fake) => fake,
        }
    }

    #[cfg(test)]
    pub(crate) fn create_test_instance() -> Self {
        Self::create_test_instance_with_channel_id(42)
    }

    #[cfg(test)]
    pub(crate) fn create_test_instance_with_channel_id(channel_id: usize) -> Self {
        use std::{
            net::{IpAddr, Ipv6Addr, SocketAddr},
            sync::Arc,
        };

        use crate::{stats::Stats, utils::StubIoContext};

        let limiter = Arc::new(OutboundBandwidthLimiter::default());
        let io_ctx = Box::new(StubIoContext::new());
        let stats = Arc::new(Stats::default());

        Self::Fake(ChannelFake::new(
            SystemTime::now(),
            channel_id,
            io_ctx,
            limiter,
            stats,
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 123),
            3,
        ))
    }
}
