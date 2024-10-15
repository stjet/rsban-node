use rsnano_network::ChannelInfo;
use std::sync::Arc;

pub(crate) struct Channels(Vec<Arc<ChannelInfo>>);

impl Channels {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn update(&mut self, channels: Vec<Arc<ChannelInfo>>) {
        self.0 = channels;
    }

    pub(crate) fn get(&self, index: usize) -> Option<&ChannelInfo> {
        self.0.get(index).map(|c| &**c)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChannelInfo> {
        self.0.iter().map(|c| &**c)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
