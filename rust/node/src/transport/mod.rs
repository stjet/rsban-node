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
mod token_bucket;
mod write_queue;

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
pub use tcp_channels::TcpChannels;
pub use tcp_message_manager::{TcpMessageItem, TcpMessageManager};
pub use tcp_server::{
    BootstrapMessageVisitor, HandshakeMessageVisitor, HandshakeMessageVisitorImpl,
    RealtimeMessageVisitor, RealtimeMessageVisitorImpl, RequestResponseVisitorFactory, TcpServer,
    TcpServerExt, TcpServerObserver,
};
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
    fn get_last_bootstrap_attempt(&self) -> u64;
    fn set_last_bootstrap_attempt(&self, instant: u64);
    fn get_last_packet_received(&self) -> u64;
    fn set_last_packet_received(&self, instant: u64);
    fn get_last_packet_sent(&self) -> u64;
    fn set_last_packet_sent(&self, instant: u64);
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
}
