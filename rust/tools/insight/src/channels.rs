use rsnano_network::{ChannelId, ChannelInfo};
use std::sync::Arc;

pub(crate) struct Channels {
    channels: Vec<Arc<ChannelInfo>>,
    selected: Option<ChannelId>,
    selected_index: Option<usize>,
}

impl Channels {
    pub(crate) fn new() -> Self {
        Self {
            channels: Vec::new(),
            selected: None,
            selected_index: None,
        }
    }

    pub(crate) fn update(&mut self, channels: Vec<Arc<ChannelInfo>>) {
        self.channels = channels;
        if let Some(channel_id) = self.selected {
            match self
                .channels
                .iter()
                .enumerate()
                .find(|(_, channel)| channel.channel_id() == channel_id)
            {
                Some((i, _)) => self.selected_index = Some(i),
                None => {
                    self.selected = None;
                    self.selected_index = None;
                }
            }
        }
    }

    pub(crate) fn get(&self, index: usize) -> Option<&ChannelInfo> {
        self.channels.get(index).map(|c| &**c)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChannelInfo> {
        self.channels.iter().map(|c| &**c)
    }

    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        let Some(channel) = self.channels.get(index) else {
            return;
        };
        if self.selected == Some(channel.channel_id()) {
            self.selected = None;
            self.selected_index = None;
        } else {
            self.selected = Some(channel.channel_id());
            self.selected_index = Some(index);
        }
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_collection::MessageCollection;
    use std::sync::RwLock;

    #[test]
    fn when_channel_selected_set_message_filter() {
        let mut messages = RwLock::new(MessageCollection::default());
        let mut channels = Channels::new();
    }
}
