use std::{
    net::SocketAddr,
    sync::{atomic::AtomicU64, Arc},
};

use anyhow::Result;

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

pub struct Socket {
    /// The other end of the connection
    pub remote: Option<SocketAddr>,

    /// the timestamp (in seconds since epoch) of the last time there was successful activity on the socket
    /// activity is any successful connect, send or receive event
    pub last_completion_time_or_init: AtomicU64,

    stats: Arc<Stat>,
}

impl Socket {
    pub fn new(stats: Arc<Stat>) -> Self {
        Self {
            remote: None,
            last_completion_time_or_init: AtomicU64::new(seconds_since_epoch()),
            stats,
        }
    }

    pub fn async_connect(
        &mut self,
        endpoint: SocketAddr,
        ec: ErrorCode,
        callback: Box<dyn Fn(ErrorCode)>,
    ) -> Result<()> {
        if ec.is_err() {
            self.stats
                .inc(StatType::Tcp, DetailType::TcpConnectError, Direction::In)?;
        } else {
            self.set_last_completion()
        }
        self.remote = Some(endpoint);
        callback(ec);
        Ok(())
    }

    pub fn set_last_completion(&self) {
        self.last_completion_time_or_init
            .store(seconds_since_epoch(), std::sync::atomic::Ordering::SeqCst);
    }
}
