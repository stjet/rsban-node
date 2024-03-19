use rsnano_core::{Account, Vote};
use rsnano_messages::{ConfirmAck, Message};

use super::VoteProcessorQueue;
use crate::{
    config::NetworkConstants,
    representatives::RepresentativeRegister,
    stats::Stats,
    transport::{
        BufferDropPolicy, ChannelEnum, ChannelInProc, InboundCallback, OutboundBandwidthLimiter,
        TcpChannels, TrafficType,
    },
    utils::AsyncRuntime,
};
use std::{
    net::SocketAddrV6,
    ops::Deref,
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct VoteBroadcaster {
    pub representative_register: Arc<Mutex<RepresentativeRegister>>,
    pub tcp_channels: Arc<TcpChannels>,
    pub vote_processor_queue: Arc<VoteProcessorQueue>,
    pub network_constants: NetworkConstants,
    pub stats: Arc<Stats>,
    pub async_rt: Arc<AsyncRuntime>,
    pub node_id: Account,
    pub local_endpoint: SocketAddrV6,
    pub inbound: InboundCallback,
}

impl VoteBroadcaster {
    pub fn broadcast(&self, vote: Arc<Vote>) {
        self.flood_vote_pr(vote.deref().clone());

        let ack = Message::ConfirmAck(ConfirmAck::new(vote.deref().clone()));
        self.tcp_channels.flood_message(&ack, 2.0);

        let loopback_channel = ChannelInProc::new(
            self.tcp_channels.get_next_channel_id(),
            SystemTime::now(),
            self.network_constants.clone(),
            Arc::clone(&self.tcp_channels.publish_filter),
            Arc::clone(&self.stats),
            Arc::new(OutboundBandwidthLimiter::default()),
            Arc::clone(&self.inbound),
            Arc::clone(&self.inbound),
            &self.async_rt,
            self.local_endpoint,
            self.local_endpoint,
            self.node_id,
            self.node_id,
        );

        self.vote_processor_queue
            .vote(&vote, &Arc::new(ChannelEnum::InProc(loopback_channel)));
    }

    fn flood_vote_pr(&self, vote: Vote) {
        let message = Message::ConfirmAck(ConfirmAck::new(vote));
        for rep in self
            .representative_register
            .lock()
            .unwrap()
            .representatives()
        {
            rep.channel.send(
                &message,
                None,
                BufferDropPolicy::NoLimiterDrop,
                TrafficType::Generic,
            )
        }
    }
}
