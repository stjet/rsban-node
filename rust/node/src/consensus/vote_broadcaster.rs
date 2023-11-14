use super::Vote;
use crate::{
    messages::{ConfirmAck, Message},
    representatives::RepresentativeRegister,
    transport::{BufferDropPolicy, TrafficType},
};
use std::sync::{Arc, Mutex};

pub struct VoteBroadcaster {
    pub representative_register: Arc<Mutex<RepresentativeRegister>>,
}

impl VoteBroadcaster {
    pub fn broadcast(&self, vote: Arc<Vote>) {
        self.flood_vote_pr(vote);
    }

    fn flood_vote_pr(&self, vote: Arc<Vote>) {
        let message = Message::ConfirmAck(ConfirmAck { vote });
        for rep in self
            .representative_register
            .lock()
            .unwrap()
            .representatives()
        {
            rep.channel().send(
                &message,
                None,
                BufferDropPolicy::NoLimiterDrop,
                TrafficType::Generic,
            )
        }
    }
}
