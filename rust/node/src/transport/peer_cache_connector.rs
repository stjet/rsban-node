use crate::{transport::TcpChannelsExtension, utils::Runnable};
use rsnano_ledger::Ledger;
use std::{net::SocketAddrV6, sync::Arc, time::SystemTime};
use tracing::info;

use super::TcpChannels;

// Tries to connect to peers that are stored in the peer cache
pub struct PeerCacheConnector {
    ledger: Arc<Ledger>,
    channels: Arc<TcpChannels>,
    first_run: bool,
}

impl PeerCacheConnector {
    pub fn new(ledger: Arc<Ledger>, channels: Arc<TcpChannels>) -> Self {
        Self {
            ledger,
            channels,
            first_run: true,
        }
    }
}

impl Runnable for PeerCacheConnector {
    fn run(&mut self) {
        let cached_peers: Vec<(SocketAddrV6, SystemTime)> = {
            let tx = self.ledger.read_txn();
            self.ledger.store.peer.iter(&tx).collect()
        };

        if self.first_run {
            info!("Adding cached initial peers: {}", cached_peers.len());
            self.first_run = false;
        }

        for (peer, _) in cached_peers {
            self.channels.merge_peer(peer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::parse_endpoint;
    use std::time::UNIX_EPOCH;
    use tracing_test::traced_test;

    #[test]
    fn no_cached_peers() {
        let ledger = Arc::new(Ledger::new_null());
        let channels = Arc::new(TcpChannels::new_null());
        let merge_tracker = channels.track_merge_peer();
        let mut connector = PeerCacheConnector::new(ledger, channels);

        connector.run();

        assert_eq!(merge_tracker.output(), Vec::new());
    }

    #[test]
    fn connect_to_cached_peers() {
        let peer1 = parse_endpoint("[::ffff:10.0.0.1]:1234");
        let peer2 = parse_endpoint("[::ffff:10.0.0.2]:1234");
        let ledger = Arc::new(
            Ledger::new_null_builder()
                .peers([(peer1, UNIX_EPOCH), (peer2, UNIX_EPOCH)])
                .finish(),
        );
        let channels = Arc::new(TcpChannels::new_null());
        let merge_tracker = channels.track_merge_peer();
        let mut connector = PeerCacheConnector::new(ledger, channels);

        connector.run();

        assert_eq!(merge_tracker.output(), [peer1, peer2]);
    }

    #[test]
    #[traced_test]
    fn log_initial_peers() {
        let peer1 = parse_endpoint("[::ffff:10.0.0.1]:1234");
        let peer2 = parse_endpoint("[::ffff:10.0.0.2]:1234");
        let ledger = Arc::new(
            Ledger::new_null_builder()
                .peers([(peer1, UNIX_EPOCH), (peer2, UNIX_EPOCH)])
                .finish(),
        );
        let channels = Arc::new(TcpChannels::new_null());
        let mut connector = PeerCacheConnector::new(ledger, channels);

        connector.run();

        assert!(logs_contain("Adding cached initial peers: 2"));
    }

    #[test]
    #[traced_test]
    fn log_initial_peers_only_once() {
        let ledger = Arc::new(Ledger::new_null());
        let channels = Arc::new(TcpChannels::new_null());
        let mut connector = PeerCacheConnector::new(ledger, channels);

        connector.run();
        connector.run();

        logs_assert(|lines| {
            match lines
                .iter()
                .filter(|l| l.contains("Adding cached initial peers"))
                .count()
            {
                1 => Ok(()),
                c => Err(format!("Should only log once, but was {}", c)),
            }
        })
    }
}
