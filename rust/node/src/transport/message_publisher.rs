use super::{Channel, ChannelId, DropPolicy, Network, TrafficType};
use crate::representatives::OnlineReps;
use rsnano_messages::Message;
use std::sync::{Arc, Mutex};

/// Publishes messages to peered nodes
#[derive(Clone)]
pub struct MessagePublisher {
    online_reps: Arc<Mutex<OnlineReps>>,
    network: Arc<Network>,
}

impl MessagePublisher {
    pub(crate) fn new(online_reps: Arc<Mutex<OnlineReps>>, network: Arc<Network>) -> Self {
        Self {
            online_reps,
            network,
        }
    }

    pub fn try_send(
        &self,
        channel_id: ChannelId,
        message: &Message,
        drop_policy: DropPolicy,
        traffic_type: TrafficType,
    ) {
        self.network
            .try_send(channel_id, message, drop_policy, traffic_type)
    }

    pub(crate) fn flood_prs_and_some_non_prs(
        &mut self,
        message: &Message,
        drop_policy: DropPolicy,
        traffic_type: TrafficType,
        scale: f32,
    ) {
        for rep in self.online_reps.lock().unwrap().peered_principal_reps() {
            self.network
                .try_send(rep.channel_id, &message, drop_policy, traffic_type)
        }

        for peer in self.list_no_pr(self.network.fanout(scale)) {
            peer.try_send(&message, drop_policy, traffic_type)
        }
    }

    fn list_no_pr(&self, count: usize) -> Vec<Arc<Channel>> {
        let mut channels = self.network.random_list_realtime(usize::MAX, 0);
        {
            let reps = self.online_reps.lock().unwrap();
            channels.retain(|c| !reps.is_pr(c.channel_id()));
        }
        channels.truncate(count);
        channels
    }
}
