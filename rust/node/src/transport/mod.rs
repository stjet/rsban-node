mod bandwidth_limiter;
mod block_deserializer;
mod channel;
mod dead_channel_cleanup;
mod fair_queue;
mod handshake_process;
mod inbound_message_queue;
mod latest_keepalives;
mod message_deserializer;
mod message_processor;
mod message_publisher;
mod network;
mod network_filter;
mod network_info;
mod network_stats;
mod network_threads;
mod peer_cache_connector;
mod peer_cache_updater;
mod peer_connector;
mod realtime_message_handler;
mod response_server;
mod response_server_factory;
mod syn_cookies;
mod tcp_listener;
mod token_bucket;
mod tokio_socket_facade;
mod vec_buffer_reader;
mod write_queue;

pub use bandwidth_limiter::{
    BandwidthLimitType, BandwidthLimiter, OutboundBandwidthLimiter, OutboundBandwidthLimiterConfig,
};
pub use block_deserializer::read_block;
pub use channel::*;
pub(crate) use dead_channel_cleanup::*;
pub(crate) use fair_queue::*;
pub(crate) use handshake_process::*;
pub use inbound_message_queue::InboundMessageQueue;
pub use latest_keepalives::*;
pub use message_deserializer::{AsyncBufferReader, MessageDeserializer};
pub use message_processor::*;
pub use message_publisher::*;
pub use network::*;
pub use network_filter::NetworkFilter;
pub use network_info::*;
pub use network_stats::*;
pub(crate) use network_threads::*;
pub use peer_cache_connector::*;
pub use peer_cache_updater::*;
pub use peer_connector::*;
pub use realtime_message_handler::RealtimeMessageHandler;
pub use response_server::*;
pub(crate) use response_server_factory::*;
use rsnano_network::ChannelDirection;
use std::fmt::Debug;
pub use syn_cookies::SynCookies;
pub use tcp_listener::*;
use token_bucket::TokenBucket;
pub use tokio_socket_facade::*;
pub use vec_buffer_reader::VecBufferReader;

use crate::stats;

/// Policy to affect at which stage a buffer can be dropped
#[derive(PartialEq, Eq, FromPrimitive, Debug, Clone, Copy)]
pub enum DropPolicy {
    /// Can be dropped by bandwidth limiter (default)
    CanDrop,
    /// Should not be dropped by bandwidth limiter,
    /// but it can still be dropped if the write queue is full
    ShouldNotDrop,
}

impl From<ChannelDirection> for stats::Direction {
    fn from(value: ChannelDirection) -> Self {
        match value {
            ChannelDirection::Inbound => stats::Direction::In,
            ChannelDirection::Outbound => stats::Direction::Out,
        }
    }
}
