use std::sync::Arc;

use crate::network::ChannelTcp;

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
}

impl BootstrapClient {
    pub fn new(observer: Arc<dyn BootstrapClientObserver>, channel: Arc<ChannelTcp>) -> Self {
        channel.set_endpoint();
        Self {
            observer: observer.to_weak(),
            channel,
        }
    }

    pub fn get_channel(&self) -> &Arc<ChannelTcp> {
        &self.channel
    }
}

impl Drop for BootstrapClient {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade() {
            observer.bootstrap_client_closed();
        }
    }
}
