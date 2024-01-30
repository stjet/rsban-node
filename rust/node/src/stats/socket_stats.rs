use super::{DetailType, Direction, StatType, Stats};
use crate::transport::{EndpointType, SocketObserver};
use rsnano_core::utils::{LogType, Logger};
use std::{net::SocketAddrV6, sync::Arc};

pub struct SocketStats {
    stats: Arc<Stats>,
    logger: Arc<dyn Logger>,
    enable_timeout_logging: bool,
}

impl SocketStats {
    pub fn new(stats: Arc<Stats>, logger: Arc<dyn Logger>, enable_timeout_logging: bool) -> Self {
        Self {
            stats,
            logger,
            enable_timeout_logging,
        }
    }
}

impl SocketObserver for SocketStats {
    fn disconnect_due_to_timeout(&self, endpoint: SocketAddrV6) {
        self.logger.debug(
            LogType::TcpServer,
            &format!("Closing socket due to timeout ({})", endpoint),
        );
    }

    fn connect_error(&self) {
        let _ = self
            .stats
            .inc(StatType::Tcp, DetailType::TcpConnectError, Direction::In);
    }

    fn read_error(&self) {
        let _ = self
            .stats
            .inc(StatType::Tcp, DetailType::TcpReadError, Direction::In);
    }

    fn read_successful(&self, len: usize) {
        let _ = self.stats.add(
            StatType::TrafficTcp,
            DetailType::All,
            Direction::In,
            len as u64,
            false,
        );
    }

    fn write_error(&self) {
        let _ = self
            .stats
            .inc(StatType::Tcp, DetailType::TcpWriteError, Direction::In);
    }

    fn write_successful(&self, len: usize) {
        let _ = self.stats.add(
            StatType::TrafficTcp,
            DetailType::All,
            Direction::Out,
            len as u64,
            false,
        );
    }

    fn silent_connection_dropped(&self) {
        let _ = self.stats.inc(
            StatType::Tcp,
            DetailType::TcpSilentConnectionDrop,
            Direction::In,
        );
    }

    fn inactive_connection_dropped(&self, endpoint_type: EndpointType) {
        let _ = self.stats.inc(
            StatType::Tcp,
            DetailType::TcpIoTimeoutDrop,
            if endpoint_type == EndpointType::Server {
                Direction::In
            } else {
                Direction::Out
            },
        );
    }
}
