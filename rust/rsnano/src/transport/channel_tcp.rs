use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, MutexGuard, Weak,
    },
};

use super::{BufferDropPolicy, Channel, Socket, SocketImpl};
use crate::{
    ffi::ChannelTcpObserverWeakPtr,
    messages::Message,
    utils::{ErrorCode, IoContext},
    Account, BandwidthLimiter,
};

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
    /* Mark for temporary channels. Usually remote ports of these channels are ephemeral and received from incoming connections to server.
    If remote part has open listening port, temporary channel will be replaced with direct connection to listening port soon.
    But if other side is behing NAT or firewall this connection can be pemanent. */
    temporary: AtomicBool,
    network_version: AtomicU8,
    pub observer: ChannelTcpObserverWeakPtr,
    pub limiter: Arc<BandwidthLimiter>,
    pub io_ctx: Arc<dyn IoContext>,
}

impl ChannelTcp {
    pub fn new(
        socket: &Arc<SocketImpl>,
        now: u64,
        observer: ChannelTcpObserverWeakPtr,
        limiter: Arc<BandwidthLimiter>,
        io_ctx: Arc<dyn IoContext>,
    ) -> Self {
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
            limiter,
            io_ctx,
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

    pub fn send_buffer(
        &self,
        buffer_a: &Arc<Vec<u8>>,
        callback_a: Box<dyn FnOnce(ErrorCode, usize)>,
        policy_a: BufferDropPolicy,
    ) {
        if let Some(socket_l) = self.socket() {
            if !socket_l.max() || (policy_a == BufferDropPolicy::NoSocketDrop && !socket_l.full()) {

                // std::weak_ptr<nano::transport::channel_tcp_observer> observer_weak_l;
                // auto observer_l = get_observer ();
                // if (observer_l)
                // {
                //     observer_weak_l = observer_l;
                // }

                //TODO:
                //socket_l.async_write(Arc::new(), callback)

                // socket_l->async_write (
                // buffer_a, [endpoint_a = socket_l->remote_endpoint (), callback_a, observer_a = observer_weak_l] (boost::system::error_code const & ec, std::size_t size_a) {
                //     if (auto observer_l = observer_a.lock ())
                //     {
                //         if (!ec)
                //         {
                //             observer_l->data_sent (endpoint_a);
                //         }
                //         if (ec == boost::system::errc::host_unreachable)
                //         {
                //             observer_l->host_unreachable ();
                //         }
                //     }
                //     if (callback_a)
                //     {
                //         callback_a (ec, size_a);
                //     }
                // });
            } else {
                // if (auto observer_l = get_observer ())
                // {
                //     if (policy_a == nano::buffer_drop_policy::no_socket_drop)
                //     {
                //         observer_l->no_socket_drop ();
                //     }
                //     else
                //     {
                //         observer_l->write_drop ();
                //     }
                // }
                // if (callback_a)
                // {
                //     callback_a (boost::system::errc::make_error_code (boost::system::errc::no_buffer_space), 0);
                // }
            }
        }
        // else if (callback_a)
        // {
        // 	get_io_ctx ()->post ([callback_a] () {
        // 		callback_a (boost::system::errc::make_error_code (boost::system::errc::not_supported), 0);
        // 	});
        // }
        todo!()
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
