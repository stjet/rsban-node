use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    logger_mt::Logger,
    messages::Message,
    transport::{Socket, SocketImpl},
    NodeConfig,
};

pub struct BootstrapServer {
    socket: Arc<SocketImpl>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
    stopped: AtomicBool,
    pub queue: Mutex<VecDeque<Box<dyn Message>>>,
}

impl BootstrapServer {
    pub fn new(socket: Arc<SocketImpl>, config: Arc<NodeConfig>, logger: Arc<dyn Logger>) -> Self {
        Self {
            socket,
            config,
            logger,
            stopped: AtomicBool::new(false),
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.socket.close();
        }
    }
}

impl Drop for BootstrapServer {
    fn drop(&mut self) {
        self.stop();
    }
}
