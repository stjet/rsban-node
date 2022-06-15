use std::sync::atomic::{AtomicBool, Ordering};

use super::Channel;

pub struct ChannelInProc{
    temporary: AtomicBool,
}

impl ChannelInProc {
    pub fn new() -> Self { Self { temporary: AtomicBool::new(false)  } }
}

impl Channel for ChannelInProc{
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, std::sync::atomic::Ordering::SeqCst);
    }
}