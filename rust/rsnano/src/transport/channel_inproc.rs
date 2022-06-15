use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use super::Channel;

pub struct InProcChannelData {
    last_bootstrap_attempt: u64,
}

pub struct ChannelInProc {
    temporary: AtomicBool,
    channel_mutex: Mutex<InProcChannelData>,
}

impl ChannelInProc {
    pub fn new() -> Self {
        Self {
            temporary: AtomicBool::new(false),
            channel_mutex: Mutex::new(InProcChannelData {
                last_bootstrap_attempt: 0,
            }),
        }
    }
}

impl Channel for ChannelInProc {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary
            .store(temporary, std::sync::atomic::Ordering::SeqCst);
    }

    fn get_last_bootstrap_attempt(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt
    }

    fn set_last_bootstrap_attempt(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt = instant;
    }
}
