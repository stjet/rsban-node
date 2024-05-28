use crate::{
    block_processing::{BlockProcessor, BlockSource},
    bootstrap::{BootstrapAscending, BootstrapServer},
    config::{NodeConfig, NodeFlags},
    consensus::{RequestAggregator, VoteProcessorQueue},
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, TcpChannelsExtension, TrafficType},
    wallets::Wallets,
    Telemetry,
};
use rsnano_messages::{Message, TelemetryAck};
use std::{net::SocketAddrV6, sync::Arc};
use tracing::trace;

use super::{ChannelEnum, TcpChannels};

pub struct LiveMessageProcessor {
    stats: Arc<Stats>,
    channels: Arc<TcpChannels>,
    block_processor: Arc<BlockProcessor>,
    config: NodeConfig,
    flags: NodeFlags,
    wallets: Arc<Wallets>,
    request_aggregator: Arc<RequestAggregator>,
    vote_processor_queue: Arc<VoteProcessorQueue>,
    telemetry: Arc<Telemetry>,
    bootstrap_server: Arc<BootstrapServer>,
    ascend_boot: Arc<BootstrapAscending>,
}

impl LiveMessageProcessor {
    pub fn new(
        stats: Arc<Stats>,
        channels: Arc<TcpChannels>,
        block_processor: Arc<BlockProcessor>,
        config: NodeConfig,
        flags: NodeFlags,
        wallets: Arc<Wallets>,
        request_aggregator: Arc<RequestAggregator>,
        vote_processor_queue: Arc<VoteProcessorQueue>,
        telemetry: Arc<Telemetry>,
        bootstrap_server: Arc<BootstrapServer>,
        ascend_boot: Arc<BootstrapAscending>,
    ) -> Self {
        Self {
            stats,
            channels,
            block_processor,
            config,
            flags,
            wallets,
            request_aggregator,
            vote_processor_queue,
            telemetry,
            bootstrap_server,
            ascend_boot,
        }
    }

    pub fn process(&self, message: Message, channel: &Arc<ChannelEnum>) {
        self.stats.inc_dir(
            StatType::Message,
            message.message_type().into(),
            Direction::In,
        );
        trace!(?message, "network processed");

        match message {
            Message::Keepalive(keepalive) => {
                // Check for special node port data
                let peer0 = keepalive.peers[0];
                if peer0.ip().is_unspecified() && peer0.port() != 0 {
                    let new_endpoint =
                        SocketAddrV6::new(*channel.remote_endpoint().ip(), peer0.port(), 0, 0);
                    self.channels.merge_peer(new_endpoint);

                    // Remember this for future forwarding to other peers
                    channel.set_peering_endpoint(new_endpoint);
                }
            }
            Message::Publish(publish) => {
                let added = self.block_processor.add(
                    Arc::new(publish.block),
                    BlockSource::Live,
                    Some(Arc::clone(channel)),
                );
                if !added {
                    self.channels.publish_filter.clear(publish.digest);
                    self.stats
                        .inc_dir(StatType::Drop, DetailType::Publish, Direction::In);
                }
            }
            Message::ConfirmReq(req) => {
                // Don't load nodes with disabled voting
                if self.config.enable_voting && self.wallets.voting_reps_count() > 0 {
                    if !req.roots_hashes().is_empty() {
                        self.request_aggregator
                            .add(Arc::clone(channel), req.roots_hashes());
                    }
                }
            }
            Message::ConfirmAck(ack) => {
                if !ack.vote().voting_account.is_zero() {
                    self.vote_processor_queue
                        .vote(&Arc::new(ack.vote().clone()), channel);
                }
            }
            Message::NodeIdHandshake(_) => {
                self.stats.inc_dir(
                    StatType::Message,
                    DetailType::NodeIdHandshake,
                    Direction::In,
                );
            }
            Message::TelemetryReq => {
                // Send an empty telemetry_ack if we do not want, just to acknowledge that we have received the message to
                // remove any timeouts on the server side waiting for a message.
                let data = if !self.flags.disable_providing_telemetry_metrics {
                    let telemetry_data = self.telemetry.local_telemetry();
                    Some(telemetry_data)
                } else {
                    None
                };

                let msg = Message::TelemetryAck(TelemetryAck(data));
                channel.send(
                    &msg,
                    None,
                    BufferDropPolicy::NoSocketDrop,
                    TrafficType::Generic,
                );
            }
            Message::TelemetryAck(ack) => self.telemetry.process(&ack, channel),
            Message::AscPullReq(req) => {
                self.bootstrap_server.request(req, Arc::clone(channel));
            }
            Message::AscPullAck(ack) => self.ascend_boot.process(&ack, channel),
            Message::FrontierReq(_)
            | Message::BulkPush
            | Message::BulkPull(_)
            | Message::BulkPullAccount(_) => unreachable!(),
        }
    }
}
