use std::{
    net::SocketAddr,
    sync::{atomic::AtomicU64, Arc, Mutex, Weak},
    time::Duration,
};

use crate::{
    seconds_since_epoch,
    stats::{DetailType, Direction, Stat, StatType},
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

pub struct SocketImpl {
    /// The other end of the connection
    pub remote: Option<SocketAddr>,

    /// the timestamp (in seconds since epoch) of the last time there was successful activity on the socket
    /// activity is any successful connect, send or receive event
    pub last_completion_time_or_init: AtomicU64,

    pub default_timeout: Duration,

    /// Duration of inactivity that causes a socket timeout
    /// activity is any successful connect, send or receive event
    pub timeout: Duration,

    tcp_socket: Arc<dyn TcpSocketFacade>,
    stats: Arc<Stat>,
}

impl SocketImpl {
    pub fn new(
        tcp_socket: Arc<dyn TcpSocketFacade>,
        stats: Arc<Stat>,
        default_timeout: Duration,
    ) -> Self {
        Self {
            remote: None,
            last_completion_time_or_init: AtomicU64::new(seconds_since_epoch()),
            tcp_socket,
            default_timeout,
            timeout: Duration::MAX,
            stats,
        }
    }

    pub fn set_last_completion(&self) {
        self.last_completion_time_or_init
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
        checkup(Arc::downgrade(self));

        let self_clone = self.clone();
        let mut lock = self.lock().unwrap();
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

fn checkup(_socket: Weak<Mutex<SocketImpl>>) {}
