use crate::message_collection::MessageCollection;
use rsnano_messages::TelemetryData;
use rsnano_network::{ChannelDirection, ChannelId, ChannelInfo};
use std::{
    collections::HashMap,
    net::SocketAddrV6,
    sync::{Arc, RwLock},
};

pub(crate) struct Channel {
    pub channel_id: ChannelId,
    pub remote_addr: SocketAddrV6,
    pub direction: ChannelDirection,
    pub telemetry: Option<TelemetryData>,
}

pub(crate) struct Channels {
    channels: Vec<Channel>,
    selected: Option<ChannelId>,
    selected_index: Option<usize>,
    messages: Arc<RwLock<MessageCollection>>,
}

impl Channels {
    pub(crate) fn new(messages: Arc<RwLock<MessageCollection>>) -> Self {
        Self {
            channels: Vec::new(),
            selected: None,
            selected_index: None,
            messages,
        }
    }

    pub(crate) fn update(
        &mut self,
        channels: Vec<Arc<ChannelInfo>>,
        telemetries: HashMap<SocketAddrV6, TelemetryData>,
    ) {
        let mut insert = Vec::new();
        {
            let mut pending: HashMap<ChannelId, &mut Channel> = self
                .channels
                .iter_mut()
                .map(|c| (c.channel_id, c))
                .collect();

            for info in channels {
                if let Some(channel) = pending.remove(&info.channel_id()) {
                    channel.telemetry = telemetries.get(&channel.remote_addr).cloned();
                } else {
                    insert.push(Channel {
                        channel_id: info.channel_id(),
                        remote_addr: info.peer_addr(),
                        direction: info.direction(),
                        telemetry: None,
                    });
                }
            }

            let to_remove: Vec<_> = pending.keys().cloned().collect();
            for key in to_remove {
                self.channels.retain(|c| c.channel_id != key);
            }
        }

        if insert.len() > 0 {
            for channel in insert {
                self.channels.push(channel);
            }
            self.channels.sort_by_key(|c| c.remote_addr);

            // Recalculate selected index
            if let Some(channel_id) = self.selected {
                match self
                    .channels
                    .iter()
                    .enumerate()
                    .find(|(_, channel)| channel.channel_id == channel_id)
                {
                    Some((i, _)) => self.selected_index = Some(i),
                    None => {
                        self.selected = None;
                        self.selected_index = None;
                    }
                }
            }
        }
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Channel> {
        self.channels.get(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Channel> {
        self.channels.iter()
    }

    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        let Some(channel) = self.channels.get(index) else {
            return;
        };
        if self.selected == Some(channel.channel_id) {
            self.selected = None;
            self.selected_index = None;
            self.messages.write().unwrap().filter_channel(None)
        } else {
            self.selected = Some(channel.channel_id);
            self.selected_index = Some(index);
            self.messages
                .write()
                .unwrap()
                .filter_channel(Some(channel.channel_id))
        }
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_collection::{MessageCollection, MessageFilter};
    use std::{collections::HashMap, sync::RwLock};

    #[test]
    fn when_channel_selected_should_set_message_filter() {
        let messages = Arc::new(RwLock::new(MessageCollection::default()));
        let mut channels = Channels::new(messages.clone());
        channels.update(
            vec![Arc::new(ChannelInfo::new_test_instance())],
            HashMap::new(),
        );
        channels.select_index(0);
        let guard = messages.read().unwrap();
        assert_eq!(
            guard.current_filter(),
            &MessageFilter::channel(channels.channels[0].channel_id)
        );
    }

    #[test]
    fn when_channel_deselected_should_clear_message_filter() {
        let messages = Arc::new(RwLock::new(MessageCollection::default()));
        let mut channels = Channels::new(messages.clone());
        channels.update(
            vec![Arc::new(ChannelInfo::new_test_instance())],
            HashMap::new(),
        );
        channels.select_index(0);
        channels.select_index(0);
        let guard = messages.read().unwrap();
        assert_eq!(guard.current_filter(), &MessageFilter::all());
    }
}
