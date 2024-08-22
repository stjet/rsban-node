use tracing::debug;

use super::{ChannelDirection, NetworkError};
use crate::stats::{DetailType, Direction, StatType, Stats};
use std::{net::SocketAddrV6, sync::Arc};

#[derive(Clone)]
pub struct NetworkStats {
    stats: Arc<Stats>,
}

impl NetworkStats {
    pub fn new(stats: Arc<Stats>) -> Self {
        Self { stats }
    }

    pub fn connect(&self, peer: &SocketAddrV6) {
        self.stats.inc_dir(
            StatType::TcpListener,
            DetailType::ConnectInitiate,
            Direction::Out,
        );
        debug!(?peer, "Initiate outgoing connection");
    }

    pub fn error(&self, error: NetworkError, peer: &SocketAddrV6, direction: ChannelDirection) {
        match error {
            NetworkError::MaxConnections => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxAttempts,
                    direction.into(),
                );
                debug!(
                    %peer,
                    ?direction,
                    "Max connections reached, unable to make new connection",
                );
            }
            NetworkError::PeerExcluded => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::Excluded,
                    direction.into(),
                );
                debug!(
                    %peer,
                    ?direction,
                    "Peer excluded, unable to make new connection",
                );
            }
            NetworkError::MaxConnectionsPerSubnetwork => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerSubnetwork,
                    direction.into(),
                );
                self.stats.inc_dir(
                    StatType::Tcp,
                    DetailType::MaxPerSubnetwork,
                    direction.into(),
                );
                debug!(
                    %peer,
                    ?direction,
                    "Max connections per subnetwork reached, unable to open new connection",
                );
            }
            NetworkError::MaxConnectionsPerIp => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerIp,
                    direction.into(),
                );
                self.stats
                    .inc_dir(StatType::Tcp, DetailType::MaxPerIp, direction.into());
                debug!(
                    %peer,
                    ?direction,
                    "Max connections per IP reached, unable to open new connection");
            }
            NetworkError::InvalidIp => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::NotAPeer,
                    direction.into(),
                );
                debug!(
                    %peer,
                    ?direction,
                    "Invalid IP, unable to open new connection");
            }
            NetworkError::DuplicateConnection => {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::Duplicate,
                    direction.into(),
                );
                debug!(
                    %peer,
                    ?direction,
                    "Already connected to that peer, unable to open new connection");
            }
            NetworkError::Rejected => {}
        }
    }
}
