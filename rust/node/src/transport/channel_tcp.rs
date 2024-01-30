use std::{
    fmt::Display,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, MutexGuard, Weak,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rsnano_core::Account;
use rsnano_messages::{Message, MessageSerializer, ProtocolInfo};

use super::{
    write_queue::WriteCallback, BufferDropPolicy, Channel, OutboundBandwidthLimiter, Socket,
    SocketExtensions, TrafficType,
};
use crate::utils::{AsyncRuntime, ErrorCode};

pub trait IChannelTcpObserverWeakPtr: Send + Sync {
    fn lock(&self) -> Option<Arc<dyn ChannelTcpObserver>>;
}

pub trait ChannelTcpObserver: Send + Sync {
    fn data_sent(&self, endpoint: &SocketAddrV6);
    fn host_unreachable(&self);
    fn message_sent(&self, message: &Message);
    fn message_dropped(&self, message: &Message, buffer_size: usize);
    fn no_socket_drop(&self);
    fn write_drop(&self);
}

pub struct TcpChannelData {
    last_bootstrap_attempt: SystemTime,
    last_packet_received: SystemTime,
    last_packet_sent: SystemTime,
    node_id: Option<Account>,
    pub remote_endpoint: SocketAddrV6,
    pub peering_endpoint: Option<SocketAddrV6>,
}

pub struct ChannelTcp {
    channel_id: usize,
    channel_mutex: Mutex<TcpChannelData>,
    pub socket: Arc<Socket>,
    /* Mark for temporary channels. Usually remote ports of these channels are ephemeral and received from incoming connections to server.
    If remote part has open listening port, temporary channel will be replaced with direct connection to listening port soon.
    But if other side is behing NAT or firewall this connection can be pemanent. */
    temporary: AtomicBool,
    network_version: AtomicU8,
    pub observer: Arc<dyn IChannelTcpObserverWeakPtr>,
    pub limiter: Arc<OutboundBandwidthLimiter>,
    pub async_rt: Weak<AsyncRuntime>,
    message_serializer: Mutex<MessageSerializer>, // TODO remove mutex
}

impl ChannelTcp {
    pub fn new(
        socket: Arc<Socket>,
        now: SystemTime,
        observer: Arc<dyn IChannelTcpObserverWeakPtr>,
        limiter: Arc<OutboundBandwidthLimiter>,
        async_rt: &Arc<AsyncRuntime>,
        channel_id: usize,
        protocol: ProtocolInfo,
    ) -> Self {
        Self {
            channel_id,
            channel_mutex: Mutex::new(TcpChannelData {
                last_bootstrap_attempt: UNIX_EPOCH,
                last_packet_received: now,
                last_packet_sent: now,
                node_id: None,
                remote_endpoint: SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0),
                peering_endpoint: None,
            }),
            socket,
            temporary: AtomicBool::new(false),
            network_version: AtomicU8::new(0),
            observer,
            limiter,
            async_rt: Arc::downgrade(async_rt),
            message_serializer: Mutex::new(MessageSerializer::new(protocol)),
        }
    }

    pub fn socket(&self) -> Option<Arc<Socket>> {
        Some(Arc::clone(&self.socket))
    }

    pub fn lock(&self) -> MutexGuard<TcpChannelData> {
        self.channel_mutex.lock().unwrap()
    }

    pub fn set_network_version(&self, version: u8) {
        self.network_version.store(version, Ordering::Relaxed)
    }

    pub fn local_endpoint(&self) -> SocketAddr {
        self.socket()
            .map(|s| SocketAddr::V6(s.local_endpoint_v6()))
            .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0))
    }

    pub fn set_remote_endpoint(&self) {
        let mut lock = self.channel_mutex.lock().unwrap();
        debug_assert!(lock.remote_endpoint == SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0)); // Not initialized endpoint value
                                                                                                  // Calculate TCP socket endpoint
        if let Some(socket) = self.socket() {
            if let Some(ep) = socket.get_remote() {
                lock.remote_endpoint = ep;
            }
        }
    }

    pub fn peering_endpoint(&self) -> SocketAddrV6 {
        let lock = self.channel_mutex.lock().unwrap();
        match lock.peering_endpoint {
            Some(addr) => addr,
            None => lock.remote_endpoint,
        }
    }

    pub fn set_peering_endpoint(&self, address: SocketAddrV6) {
        let mut lock = self.channel_mutex.lock().unwrap();
        lock.peering_endpoint = Some(address);
    }

    pub fn send_buffer(
        &self,
        buffer: &Arc<Vec<u8>>,
        callback: Option<WriteCallback>,
        policy: BufferDropPolicy,
        traffic_type: TrafficType,
    ) {
        if let Some(socket_l) = self.socket() {
            if !socket_l.max(traffic_type)
                || (policy == BufferDropPolicy::NoSocketDrop && !socket_l.full(traffic_type))
            {
                let observer_weak_l = self.observer.clone();
                let endpoint = socket_l
                    .get_remote()
                    .unwrap_or_else(|| SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));
                socket_l.async_write(
                    buffer,
                    Some(Box::new(move |ec, size| {
                        if let Some(observer_l) = observer_weak_l.lock() {
                            if ec.is_ok() {
                                observer_l.data_sent(&endpoint);
                            }
                            if ec == ErrorCode::host_unreachable() {
                                observer_l.host_unreachable();
                            }
                        }
                        if let Some(callback) = callback {
                            callback(ec, size);
                        }
                    })),
                    traffic_type,
                );
            } else {
                if let Some(observer_l) = self.observer.lock() {
                    if policy == BufferDropPolicy::NoSocketDrop {
                        observer_l.no_socket_drop();
                    } else {
                        observer_l.write_drop();
                    }
                }
                if let Some(callback_a) = callback {
                    callback_a(ErrorCode::no_buffer_space(), 0);
                }
            }
        } else if let Some(callback_a) = callback {
            if let Some(async_rt) = self.async_rt.upgrade() {
                async_rt.post(Box::new(|| {
                    callback_a(ErrorCode::not_supported(), 0);
                }));
            }
        }
    }

    pub fn max(&self, traffic_type: TrafficType) -> bool {
        self.socket.max(traffic_type)
    }
}

impl Display for ChannelTcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.remote_endpoint().fmt(f)
    }
}

impl Channel for ChannelTcp {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, Ordering::SeqCst);
    }

    fn get_last_bootstrap_attempt(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt
    }

    fn set_last_bootstrap_attempt(&self, time: SystemTime) {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt = time;
    }

    fn get_last_packet_received(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_received
    }

    fn set_last_packet_received(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_received = instant;
    }

    fn get_last_packet_sent(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_sent
    }

    fn set_last_packet_sent(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_sent = instant;
    }

    fn get_node_id(&self) -> Option<Account> {
        self.channel_mutex.lock().unwrap().node_id
    }

    fn set_node_id(&self, id: Account) {
        self.channel_mutex.lock().unwrap().node_id = Some(id);
    }

    fn is_alive(&self) -> bool {
        self.socket().map(|s| s.is_alive()).unwrap_or(false)
    }

    fn channel_id(&self) -> usize {
        self.channel_id
    }

    fn get_type(&self) -> super::TransportType {
        super::TransportType::Tcp
    }

    fn remote_endpoint(&self) -> SocketAddrV6 {
        self.channel_mutex.lock().unwrap().remote_endpoint
    }

    fn network_version(&self) -> u8 {
        self.network_version.load(Ordering::Relaxed)
    }

    fn send(
        &self,
        message: &Message,
        callback: Option<WriteCallback>,
        drop_policy: BufferDropPolicy,
        traffic_type: TrafficType,
    ) {
        let buffer = {
            let mut serializer = self.message_serializer.lock().unwrap();
            let buffer = serializer.serialize(message);
            Arc::new(Vec::from(buffer)) // TODO don't copy into vec. Pass slice directly
        };
        let is_droppable_by_limiter = drop_policy == BufferDropPolicy::Limiter;
        let should_pass = self.limiter.should_pass(buffer.len(), traffic_type.into());
        if !is_droppable_by_limiter || should_pass {
            self.send_buffer(&buffer, callback, drop_policy, traffic_type);
            if let Some(observer) = self.observer.lock() {
                observer.message_sent(message);
            }
        } else {
            if let Some(observer) = self.observer.lock() {
                observer.message_dropped(message, buffer.len());
            }

            if let Some(callback) = callback {
                if let Some(async_rt) = self.async_rt.upgrade() {
                    async_rt.post(Box::new(move || {
                        callback(ErrorCode::not_supported(), 0);
                    }));
                }
            }
        }
    }
}

impl Drop for ChannelTcp {
    fn drop(&mut self) {
        // Close socket. Exception: socket is used by bootstrap_server
        if !self.temporary.load(Ordering::Relaxed) {
            self.socket.close();
        }
    }
}

impl PartialEq for ChannelTcp {
    fn eq(&self, other: &Self) -> bool {
        if Arc::as_ptr(&self.socket) != Arc::as_ptr(&other.socket) {
            return false;
        }

        true
    }
}
