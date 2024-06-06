mod bandwidth_limiter;
mod block_deserializer;
mod channel_fake;
mod channel_inproc;
mod channel_tcp;
mod connections_per_address;
mod fair_queue;
mod message_deserializer;
mod message_processor;
mod network_filter;
mod network_threads;
mod peer_cache_connector;
mod peer_cache_updater;
mod peer_exclusion;
mod socket;
mod syn_cookies;
mod tcp_channels;
mod tcp_listener;
mod tcp_message_manager;
mod tcp_server;
mod tcp_server_factory;
mod tcp_stream;
mod tcp_stream_factory;
mod token_bucket;
mod tokio_socket_facade;
mod write_queue;

pub use fair_queue::*;
pub use message_processor::LiveMessageProcessor;
pub use peer_cache_connector::*;
pub use peer_cache_updater::*;
use rsnano_messages::Message;
pub use tokio_socket_facade::*;

use std::{net::SocketAddrV6, ops::Deref, sync::Arc, time::SystemTime};

pub use bandwidth_limiter::{
    BandwidthLimitType, BandwidthLimiter, OutboundBandwidthLimiter, OutboundBandwidthLimiterConfig,
};
pub use block_deserializer::BlockDeserializer;
pub use channel_fake::ChannelFake;
pub use channel_inproc::{ChannelInProc, InboundCallback};
pub use channel_tcp::*;
pub(crate) use connections_per_address::ConnectionsPerAddress;
pub use message_deserializer::{AsyncBufferReader, MessageDeserializer};
pub use network_filter::NetworkFilter;
pub use network_threads::*;
pub use peer_exclusion::PeerExclusion;
use rsnano_core::Account;
pub use socket::*;
pub use syn_cookies::SynCookies;
pub use tcp_channels::{
    AttemptEntry, TcpChannels, TcpChannelsExtension, TcpChannelsImpl, TcpChannelsOptions,
};
pub use tcp_listener::{TcpListener, TcpListenerExt};
pub use tcp_message_manager::{TcpMessageItem, TcpMessageManager};
pub use tcp_server::*;
pub use tcp_server_factory::TcpServerFactory;
pub use tcp_stream::TcpStream;
pub use tcp_stream_factory::TcpStreamFactory;
use token_bucket::TokenBucket;
pub use write_queue::WriteCallback;

#[repr(u8)]
#[derive(FromPrimitive, PartialEq, Eq)]
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
    fn remote_endpoint(&self) -> SocketAddrV6;
    fn network_version(&self) -> u8;
    fn send(
        &self,
        message: &Message,
        callback: Option<WriteCallback>,
        drop_policy: BufferDropPolicy,
        traffic_type: TrafficType,
    );
    fn close(&self);
}

#[derive(FromPrimitive, Copy, Clone, Debug)]
pub enum TrafficType {
    Generic,
    /** For bootstrap (asc_pull_ack, asc_pull_req) traffic */
    Bootstrap,
}

pub enum ChannelEnum {
    Tcp(Arc<ChannelTcp>),
    InProc(ChannelInProc),
    Fake(ChannelFake),
}

impl ChannelEnum {
    #[cfg(test)]
    pub(crate) fn create_test_instance() -> Self {
        Self::create_test_instance_with_channel_id(42)
    }

    #[cfg(test)]
    pub(crate) fn create_test_instance_with_channel_id(channel_id: usize) -> Self {
        use crate::{stats::Stats, utils::AsyncRuntime};
        use rsnano_messages::ProtocolInfo;
        use std::net::Ipv6Addr;

        let limiter = Arc::new(OutboundBandwidthLimiter::default());
        let async_rt = Arc::new(AsyncRuntime::new(tokio::runtime::Runtime::new().unwrap()));
        let stats = Arc::new(Stats::default());

        Self::Fake(ChannelFake::new(
            SystemTime::now(),
            channel_id,
            &async_rt,
            limiter,
            stats,
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, 123, 0, 0),
            ProtocolInfo::dev_network(),
        ))
    }

    pub fn max(&self, traffic_type: TrafficType) -> bool {
        match self {
            Self::Tcp(tcp) => tcp.max(traffic_type),
            _ => false,
        }
    }
    pub fn set_peering_endpoint(&self, address: SocketAddrV6) {
        if let Self::Tcp(tcp) = self {
            tcp.set_peering_endpoint(address);
        }
    }
}

impl Deref for ChannelEnum {
    type Target = dyn Channel;

    fn deref(&self) -> &Self::Target {
        match &self {
            ChannelEnum::Tcp(tcp) => tcp,
            ChannelEnum::InProc(inproc) => inproc,
            ChannelEnum::Fake(fake) => fake,
        }
    }
}
