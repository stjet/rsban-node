use std::sync::{Arc, Mutex};

use crate::{
    network::{ChannelTcp, Socket, SocketImpl},
    utils::ErrorCode,
};

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
    receive_buffer: Arc<Mutex<Vec<u8>>>,
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
            receive_buffer: Arc::new(Mutex::new(vec![0; 256])),
        }
    }

    pub fn get_channel(&self) -> &Arc<ChannelTcp> {
        &self.channel
    }

    pub fn get_socket(&self) -> &Arc<SocketImpl> {
        &self.socket
    }

    pub fn read_async(&self, size: usize, callback: Box<dyn Fn(ErrorCode, usize)>) {
        self.socket
            .async_read2(Arc::clone(&self.receive_buffer), size, callback);
    }

    pub fn receive_buffer(&self) -> Vec<u8> {
        self.receive_buffer.lock().unwrap().clone()
    }

    pub fn receive_buffer_len(&self) -> usize {
        self.receive_buffer.lock().unwrap().len()
    }
}

impl Drop for BootstrapClient {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade() {
            observer.bootstrap_client_closed();
        }
    }
}
