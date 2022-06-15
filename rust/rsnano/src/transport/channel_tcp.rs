use std::sync::{Mutex, MutexGuard};

pub struct ChannelData {}

pub struct ChannelTcp {
    channel_mutex: Mutex<ChannelData>,
}

impl ChannelTcp {
    pub fn new() -> Self {
        Self {
            channel_mutex: Mutex::new(ChannelData {}),
        }
    }

    pub fn lock(&self) -> MutexGuard<ChannelData> {
        self.channel_mutex.lock().unwrap()
    }
}
