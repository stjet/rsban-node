use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

use crate::{
    seconds_since_epoch,
    stats::{DetailType, Direction, Stat, StatType},
    ThreadPool,
};

#[derive(Clone, Copy)]
pub struct ErrorCode {
    pub val: i32,
    pub category: u8,
}

impl ErrorCode {
    pub fn is_err(&self) -> bool {
        self.val != 0
    }
}

pub trait TcpSocketFacade {
    fn async_connect(&self, endpoint: SocketAddr, callback: Box<dyn Fn(ErrorCode)>);
}

#[derive(PartialEq, Eq, Clone, Copy, FromPrimitive)]
pub enum EndpointType {
    Server,
    Client,
}

pub struct SocketImpl {
    /// The other end of the connection
    pub remote: Option<SocketAddr>,

    /// the timestamp (in seconds since epoch) of the last time there was successful activity on the socket
    /// activity is any successful connect, send or receive event
    pub last_completion_time_or_init: AtomicU64,

    /// the timestamp (in seconds since epoch) of the last time there was successful receive on the socket
    /// successful receive includes graceful closing of the socket by the peer (the read succeeds but returns 0 bytes)
    pub last_receive_time_or_init: AtomicU64,

    pub default_timeout: Duration,

    /// Duration of inactivity that causes a socket timeout
    /// activity is any successful connect, send or receive event
    pub timeout: Duration,

    tcp_socket: Arc<dyn TcpSocketFacade>,
    stats: Arc<Stat>,
    thread_pool: Arc<dyn ThreadPool>,
    endpoint_type: EndpointType,
    /// used in real time server sockets, number of seconds of no receive traffic that will cause the socket to timeout
    pub silent_connection_tolerance_time: Duration,
}

impl SocketImpl {
    pub fn new(
        endpoint_type: EndpointType,
        tcp_socket: Arc<dyn TcpSocketFacade>,
        stats: Arc<Stat>,
        thread_pool: Arc<dyn ThreadPool>,
        default_timeout: Duration,
        silent_connection_tolerance_time: Duration,
    ) -> Self {
        Self {
            remote: None,
            last_completion_time_or_init: AtomicU64::new(seconds_since_epoch()),
            last_receive_time_or_init: AtomicU64::new(seconds_since_epoch()),
            tcp_socket,
            default_timeout,
            timeout: Duration::MAX,
            stats,
            thread_pool,
            endpoint_type,
            silent_connection_tolerance_time,
        }
    }

    pub fn set_last_completion(&self) {
        self.last_completion_time_or_init
            .store(seconds_since_epoch(), std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set_last_receive_time(&self) {
        self.last_receive_time_or_init
            .store(seconds_since_epoch(), std::sync::atomic::Ordering::SeqCst);
    }

    /// Set the current timeout of the socket.
    ///  timeout occurs when the last socket completion is more than timeout seconds in the past
    ///  timeout always applies, the socket always has a timeout
    ///  to set infinite timeout, use Duration::MAX
    ///  the function checkup() checks for timeout on a regular interval
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn set_default_timeout(&mut self) {
        self.timeout = self.default_timeout;
    }
}

pub trait Socket {
    fn async_connect(&self, endpoint: SocketAddr, callback: Box<dyn Fn(ErrorCode)>);
}

impl Socket for Arc<Mutex<SocketImpl>> {
    fn async_connect(&self, endpoint: SocketAddr, callback: Box<dyn Fn(ErrorCode)>) {
        let self_clone = self.clone();
        let mut lock = self.lock().unwrap();
        checkup(Arc::downgrade(self), lock.thread_pool.as_ref());
        lock.set_default_timeout();
        lock.tcp_socket.async_connect(
            endpoint,
            Box::new(move |ec| {
                let mut lock = self_clone.lock().unwrap();
                if !ec.is_err() {
                    lock.set_last_completion()
                }
                lock.remote = Some(endpoint);
                let stats = lock.stats.clone();
                drop(lock);

                if ec.is_err() {
                    let _ = stats.inc(StatType::Tcp, DetailType::TcpConnectError, Direction::In);
                }
                callback(ec);
            }),
        );
    }
}

fn checkup(socket: Weak<Mutex<SocketImpl>>, thread_pool: &dyn ThreadPool) {
    thread_pool.add_timed_task(
        Duration::from_secs(2),
        Box::new(move || {
            if let Some(socket) = socket.upgrade() {
                let now = seconds_since_epoch();
                let mut condition_to_disconnect = false;
                let lock = socket.lock().unwrap();

                // if this is a server socket, and no data is received for silent_connection_tolerance_time seconds then disconnect
                if lock.endpoint_type == EndpointType::Server
                    && (now - lock.last_receive_time_or_init.load(Ordering::SeqCst))
                        > lock.silent_connection_tolerance_time.as_secs()
                {
                    let _ = lock.stats.inc(
                        StatType::Tcp,
                        DetailType::TcpSilentConnectionDrop,
                        Direction::In,
                    );
                    condition_to_disconnect = true;
                }

                // // if there is no activity for timeout seconds then disconnect
                // if (now - lock.last_completion_time_or_init) > lock.timeout {
                // 	this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_io_timeout_drop,
                // 	this_l->endpoint_type () == endpoint_type_t::server ? nano::stat::dir::in : nano::stat::dir::out);
                // 	condition_to_disconnect = true;
                // }

                // if condition_to_disconnect {
                // 	if (this_l->network_timeout_logging)
                // 	{
                // 		// The remote end may have closed the connection before this side timing out, in which case the remote address is no longer available.
                // 		boost::system::error_code ec_remote_l;
                // 		boost::asio::ip::tcp::endpoint remote_endpoint_l = this_l->tcp_socket.remote_endpoint (ec_remote_l);
                // 		if (!ec_remote_l)
                // 		{
                // 			this_l->logger.try_log (boost::str (boost::format ("Disconnecting from %1% due to timeout") % remote_endpoint_l));
                // 		}
                // 	}
                // 	this_l->timed_out = true;
                // 	this_l->close ();
                // }
                // else if (!this_l->closed)
                // {
                //     checkup(Arc::downgrade(&socket), &lock.workers);
                // }
            }
        }),
    );
}
