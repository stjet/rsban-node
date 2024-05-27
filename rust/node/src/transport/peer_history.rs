use super::{ChannelEnum, TcpChannels};
use crate::stats::{DetailType, StatType, Stats};
use rsnano_core::utils::SystemTimeFactory;
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::LmdbWriteTransaction;
use std::{sync::Arc, time::Duration};
use tracing::debug;

pub struct PeerHistoryConfig {
    pub erase_cutoff: Duration,
    pub check_interval: Duration,
}

impl Default for PeerHistoryConfig {
    fn default() -> Self {
        Self {
            erase_cutoff: Duration::from_secs(60 * 60),
            check_interval: Duration::from_secs(15),
        }
    }
}

/// Writes a snapshot of the current peers to the database,
/// so that we can reconnect to them when the node is restarted
pub struct PeerHistory {
    channels: Arc<TcpChannels>,
    ledger: Arc<Ledger>,
    time_factory: SystemTimeFactory,
    stats: Arc<Stats>,
}

impl PeerHistory {
    pub fn new(
        channels: Arc<TcpChannels>,
        ledger: Arc<Ledger>,
        time_factory: SystemTimeFactory,
        stats: Arc<Stats>,
    ) -> Self {
        Self {
            channels,
            ledger,
            time_factory,
            stats,
        }
    }

    fn run(&self) {
        let live_peers = self.channels.list_channels(0, true);
        let mut tx = self.ledger.rw_txn();
        for peer in live_peers {
            self.save_peer(&mut tx, &peer);
        }

        // TODO: delete old peers
    }

    fn save_peer(&self, tx: &mut LmdbWriteTransaction, channel: &ChannelEnum) {
        let endpoint = channel.remote_endpoint();
        let exists = self.ledger.store.peer.exists(tx, &endpoint);

        self.ledger
            .store
            .peer
            .put(tx, &endpoint, self.time_factory.now());

        if !exists {
            self.stats.inc(StatType::PeerHistory, DetailType::Inserted);
            debug!("Saved new peer: {}", endpoint);
        } else {
            self.stats.inc(StatType::PeerHistory, DetailType::Updated);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Direction;
    use rsnano_core::utils::{create_test_endpoint, create_test_time, parse_endpoint};
    use std::{net::SocketAddrV6, time::SystemTime};
    use tracing_test::traced_test;

    #[test]
    fn config_defaults() {
        let config = PeerHistoryConfig::default();
        assert_eq!(config.erase_cutoff, Duration::from_secs(60 * 60));
        assert_eq!(config.check_interval, Duration::from_secs(15));
    }

    #[test]
    fn no_peers() {
        let open_channels = Vec::new();
        let already_stored = Vec::new();
        let (written, _) = run_peer_history(create_test_time(), open_channels, already_stored);
        assert_eq!(written, Vec::new());
    }

    #[test]
    fn write_one_peer() {
        let now = create_test_time();
        let endpoint = create_test_endpoint();
        let open_channels = vec![endpoint];
        let already_stored = Vec::new();

        let (written, _) = run_peer_history(now, open_channels, already_stored);

        assert_eq!(written, vec![(endpoint, now)]);
    }

    #[test]
    fn write_multiple_peers() {
        let endpoint1 = parse_endpoint("[::ffff:10.0.0.1]:1111");
        let endpoint2 = parse_endpoint("[::ffff:10.0.0.2]:2222");
        let endpoint3 = parse_endpoint("[::ffff:10.0.0.3]:3333");
        let now = create_test_time();
        let open_channels = vec![endpoint1, endpoint2, endpoint3];
        let already_stored = Vec::new();

        let (written, _) = run_peer_history(now, open_channels, already_stored);

        assert_eq!(
            written,
            vec![(endpoint1, now), (endpoint2, now), (endpoint3, now)]
        );
    }

    #[test]
    #[traced_test]
    fn log_when_new_peer_saved() {
        let endpoint = create_test_endpoint();
        let open_channels = vec![endpoint];
        let already_stored = Vec::new();

        run_peer_history(create_test_time(), open_channels, already_stored);

        assert!(logs_contain("Saved new peer: [::ffff:10.0.0.1]:1234"));
    }

    #[test]
    #[traced_test]
    fn dont_log_when_peer_updated() {
        let endpoint = parse_endpoint("[::ffff:10.0.0.1]:1234");
        let now = create_test_time();
        let open_channels = vec![endpoint];
        let already_stored = vec![(endpoint, now)];

        run_peer_history(now, open_channels, already_stored);

        logs_assert(|lines| {
            if lines.is_empty() {
                Ok(())
            } else {
                Err("log was written".to_string())
            }
        });
    }

    #[test]
    fn inc_stats_when_peer_inserted() {
        let endpoint = create_test_endpoint();
        let open_channels = vec![endpoint];
        let already_stored = Vec::new();

        let (_, stats) = run_peer_history(create_test_time(), open_channels, already_stored);
        assert_eq!(
            stats.count(StatType::PeerHistory, DetailType::Inserted, Direction::In),
            1
        );
        assert_eq!(
            stats.count(StatType::PeerHistory, DetailType::Updated, Direction::In),
            0
        );
    }

    #[test]
    fn inc_stats_when_peer_updated() {
        let endpoint = create_test_endpoint();
        let open_channels = vec![endpoint];
        let already_stored = vec![(endpoint, create_test_time())];

        let (_, stats) = run_peer_history(create_test_time(), open_channels, already_stored);
        assert_eq!(
            stats.count(StatType::PeerHistory, DetailType::Inserted, Direction::In),
            0
        );
        assert_eq!(
            stats.count(StatType::PeerHistory, DetailType::Updated, Direction::In),
            1
        );
    }

    fn run_peer_history(
        now: SystemTime,
        open_channels: Vec<SocketAddrV6>,
        already_stored: Vec<(SocketAddrV6, SystemTime)>,
    ) -> (Vec<(SocketAddrV6, SystemTime)>, Arc<Stats>) {
        let channels = Arc::new(TcpChannels::new_null());
        for endpoint in open_channels {
            channels.insert_fake(endpoint).unwrap();
        }
        let ledger = Arc::new(Ledger::new_null_builder().peers(already_stored).finish());
        let time_factory = SystemTimeFactory::new_null_with(now);
        let stats = Arc::new(Stats::default());
        let put_tracker = ledger.store.peer.track_puts();
        let peer_history = PeerHistory::new(channels, ledger, time_factory, Arc::clone(&stats));

        peer_history.run();

        (put_tracker.output(), stats)
    }
}
