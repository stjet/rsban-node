use super::VoteProcessorQueue;
use crate::{
    representatives::RepresentativeRegister,
    transport::{BufferDropPolicy, ChannelEnum, Network, TrafficType},
};
use rsnano_core::Vote;
use rsnano_messages::{ConfirmAck, Message};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

pub struct VoteBroadcaster {
    representative_register: Arc<Mutex<RepresentativeRegister>>,
    network: Arc<Network>,
    vote_processor_queue: Arc<VoteProcessorQueue>,
    loopback_channel: Arc<ChannelEnum>,
}

impl VoteBroadcaster {
    pub fn new(
        representative_register: Arc<Mutex<RepresentativeRegister>>,
        network: Arc<Network>,
        vote_processor_queue: Arc<VoteProcessorQueue>,
        loopback_channel: Arc<ChannelEnum>,
    ) -> Self {
        Self {
            representative_register,
            network,
            vote_processor_queue,
            loopback_channel,
        }
    }

    pub fn broadcast(&self, vote: Arc<Vote>) {
        self.flood_vote_pr(vote.deref().clone());

        let ack = Message::ConfirmAck(ConfirmAck::new(vote.deref().clone()));
        self.network.flood_message(&ack, 2.0);

        self.vote_processor_queue
            .vote(&vote, &self.loopback_channel);
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
