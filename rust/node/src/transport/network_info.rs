use super::ChannelId;

pub struct ChannelInfo {
    pub channel_id: ChannelId,
}

impl ChannelInfo {
    pub fn new(channel_id: ChannelId) -> Self {
        Self { channel_id }
    }
}

pub struct NetworkInfo {}

impl NetworkInfo {
    pub fn new() -> Self {
        Self {}
    }
}
