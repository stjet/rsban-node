use std::sync::atomic::{AtomicBool, Ordering};

use super::Channel;

pub struct ChannelUdp{
    temporary: AtomicBool,
}

impl ChannelUdp {
    pub fn new() -> Self { Self { temporary: AtomicBool::new(false) } }
}

impl Channel for ChannelUdp{
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, Ordering::SeqCst)
    }
}