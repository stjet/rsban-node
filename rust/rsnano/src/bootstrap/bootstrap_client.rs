use std::sync::Arc;

use crate::network::{ChannelTcp, SocketImpl};

pub trait BootstrapClientObserver {
    fn bootstrap_client_closed(&self);
    fn to_weak(&self) -> Box<dyn BootstrapClientObserverWeakPtr>;
}

pub trait BootstrapClientObserverWeakPtr {
    fn upgrade(&self) -> Option<Arc<dyn BootstrapClientObserver>>;
}

pub struct BootstrapClient {
    observer: Box<dyn BootstrapClientObserverWeakPtr>,
    channel: Arc<ChannelTcp>,
    socket: Arc<SocketImpl>,
}

impl BootstrapClient {
    pub fn new(
        observer: Arc<dyn BootstrapClientObserver>,
        channel: Arc<ChannelTcp>,
        socket: Arc<SocketImpl>,
    ) -> Self {
        channel.set_endpoint();
        Self {
            observer: observer.to_weak(),
            channel,
            socket,
        }
    }

    pub fn get_channel(&self) -> &Arc<ChannelTcp> {
        &self.channel
    }

    pub fn get_socket(&self) -> &Arc<SocketImpl> {
        &self.socket
    }
}

impl Drop for BootstrapClient {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade() {
            observer.bootstrap_client_closed();
        }
    }
}
