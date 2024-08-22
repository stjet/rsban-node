use crate::ChannelInfo;

pub trait NetworkObserver: Send + Sync {
    fn send_succeeded(&self, _buf_size: usize) {}
    fn send_failed(&self) {}
    fn read_succeeded(&self, _count: usize) {}
    fn read_failed(&self) {}
    fn channel_timed_out(&self, _channel: &ChannelInfo) {}
}

pub struct NullNetworkObserver {}

impl NullNetworkObserver {
    pub fn new() -> Self {
        Self {}
    }
}

impl NetworkObserver for NullNetworkObserver {}
