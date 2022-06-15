use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard, Weak,
};

use super::{Socket, SocketImpl};

pub trait Channel {
    fn is_temporary(&self) -> bool;
    fn set_temporary(&self, temporary: bool);
}

pub struct ChannelData {}

pub struct ChannelTcp {
    channel_mutex: Mutex<ChannelData>,
    socket: Weak<SocketImpl>,
    temporary: AtomicBool,
}

impl ChannelTcp {
    pub fn new(socket: &Arc<SocketImpl>) -> Self {
        Self {
            channel_mutex: Mutex::new(ChannelData {}),
            socket: Arc::downgrade(socket),
            temporary: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<ChannelData> {
        self.channel_mutex.lock().unwrap()
    }
}

impl Channel for ChannelTcp {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, Ordering::SeqCst);
    }
}

impl Drop for ChannelTcp {
    fn drop(&mut self) {
        // Close socket. Exception: socket is used by bootstrap_server
        if let Some(socket) = self.socket.upgrade() {
            if !self.temporary.load(Ordering::Relaxed) {
                socket.close();
            }
        }
    }
}
