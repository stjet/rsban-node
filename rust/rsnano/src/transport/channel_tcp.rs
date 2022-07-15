use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, MutexGuard, Weak,
    },
};

use super::{Channel, Socket, SocketImpl};
use crate::{ffi::ChannelTcpObserverWeakPtr, messages::Message, Account};

pub trait ChannelTcpObserver {
    fn data_sent(&self, endpoint: &SocketAddr);
    fn host_unreachable(&self);
    fn message_sent(&self, message: &dyn Message);
    fn message_dropped(&self, message: &dyn Message, buffer_size: usize);
    fn no_socket_drop(&self);
    fn write_drop(&self);
}

pub struct TcpChannelData {
    last_bootstrap_attempt: u64,
    last_packet_received: u64,
    last_packet_sent: u64,
    node_id: Option<Account>,
    pub endpoint: SocketAddr,
}

pub struct ChannelTcp {
    channel_mutex: Mutex<TcpChannelData>,
    socket: Weak<SocketImpl>,
    temporary: AtomicBool,
    network_version: AtomicU8,
    pub observer: ChannelTcpObserverWeakPtr,
}

impl ChannelTcp {
    pub fn new(socket: &Arc<SocketImpl>, now: u64, observer: ChannelTcpObserverWeakPtr) -> Self {
        Self {
            channel_mutex: Mutex::new(TcpChannelData {
                last_bootstrap_attempt: 0,
                last_packet_received: now,
                last_packet_sent: now,
                node_id: None,
                endpoint: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            }),
            socket: Arc::downgrade(socket),
            temporary: AtomicBool::new(false),
            network_version: AtomicU8::new(0),
            observer,
        }
    }

    pub fn socket(&self) -> Option<Arc<SocketImpl>> {
        self.socket.upgrade()
    }

    pub fn lock(&self) -> MutexGuard<TcpChannelData> {
        self.channel_mutex.lock().unwrap()
    }

    pub fn network_version(&self) -> u8 {
        self.network_version.load(Ordering::Relaxed)
    }

    pub fn set_network_version(&self, version: u8) {
        self.network_version.store(version, Ordering::Relaxed)
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.channel_mutex.lock().unwrap().endpoint
    }

    pub fn set_endpoint(&self) {
        let mut lock = self.channel_mutex.lock().unwrap();
        debug_assert!(lock.endpoint == SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)); // Not initialized endpoint value
                                                                                               // Calculate TCP socket endpoint
        if let Some(socket) = self.socket() {
            if let Some(ep) = socket.get_remote() {
                lock.endpoint = ep;
            }
        }
    }
}

impl Channel for ChannelTcp {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, Ordering::SeqCst);
    }

    fn get_last_bootstrap_attempt(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt
    }

    fn set_last_bootstrap_attempt(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt = instant;
    }

    fn get_last_packet_received(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_packet_received
    }

    fn set_last_packet_received(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_packet_received = instant;
    }

    fn get_last_packet_sent(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_packet_sent
    }

    fn set_last_packet_sent(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_packet_sent = instant;
    }

    fn get_node_id(&self) -> Option<Account> {
        self.channel_mutex.lock().unwrap().node_id
    }

    fn set_node_id(&self, id: Account) {
        self.channel_mutex.lock().unwrap().node_id = Some(id);
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
