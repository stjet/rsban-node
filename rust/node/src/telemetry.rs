use rsnano_messages::{Message, TelemetryAck, TelemetryData};
use std::{
    cmp::min,
    collections::{HashMap, VecDeque},
    net::SocketAddrV6,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::{
    stats::{DetailType, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TcpChannels, TrafficType},
    NetworkParams,
};

/**
 * This class periodically broadcasts and requests telemetry from peers.
 * Those intervals are configurable via `telemetry_request_interval` & `telemetry_broadcast_interval` network constants
 * Telemetry datas are only removed after becoming stale (configurable via `telemetry_cache_cutoff` network constant), so peer data will still be available for a short period after that peer is disconnected
 *
 * Requests can be disabled via `disable_ongoing_telemetry_requests` node flag
 * Broadcasts can be disabled via `disable_providing_telemetry_metrics` node flag
 *
 */
pub struct Telemetry {
    config: TelementryConfig,
    stats: Arc<Stats>,
    thread: Mutex<Option<JoinHandle<()>>>,
    condition: Condvar,
    mutex: Mutex<TelemetryImpl>,
    network_params: NetworkParams,
    channels: Arc<TcpChannels>,
    notify: Box<dyn Fn(&TelemetryData, &Arc<ChannelEnum>) + Send + Sync>,
}

impl Telemetry {
    const MAX_SIZE: usize = 1024;

    pub fn new(
        config: TelementryConfig,
        stats: Arc<Stats>,
        network_params: NetworkParams,
        channels: Arc<TcpChannels>,
        notify: Box<dyn Fn(&TelemetryData, &Arc<ChannelEnum>) + Send + Sync>,
    ) -> Self {
        Self {
            config,
            stats,
            network_params,
            channels,
            thread: Mutex::new(None),
            condition: Condvar::new(),
            mutex: Mutex::new(TelemetryImpl {
                stopped: false,
                triggered: false,
                telemetries: Default::default(),
                last_broadcast: None,
                last_request: None,
            }),
            notify,
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }

    fn verify(&self, telemetry: &TelemetryAck, channel: &Arc<ChannelEnum>) -> bool {
        let Some(data) = &telemetry.0 else {
            self.stats
                .inc(StatType::Telemetry, DetailType::EmptyPayload);
            return false;
        };

        // Check if telemetry node id matches channel node id
        if Some(data.node_id) != channel.get_node_id() {
            self.stats
                .inc(StatType::Telemetry, DetailType::NodeIdMismatch);
            return false;
        }

        // Check whether data is signed by node id presented in telemetry message
        if !data.validate_signature() {
            self.stats
                .inc(StatType::Telemetry, DetailType::InvalidSignature);
            return false;
        }

        if data.genesis_block != self.network_params.ledger.genesis.hash() {
            self.channels.exclude(channel);

            self.stats
                .inc(StatType::Telemetry, DetailType::GenesisMismatch);
            return false;
        }

        return true; // Telemetry is OK
    }

    fn process(&self, telemetry: &TelemetryAck, channel: &Arc<ChannelEnum>) {
        if !self.verify(telemetry, channel) {
            return;
        }
        let data = telemetry.0.as_ref().unwrap();

        let mut guard = self.mutex.lock().unwrap();
        let endpoint = channel.remote_endpoint();

        if let Some(entry) = guard.telemetries.get_mut(&endpoint) {
            self.stats.inc(StatType::Telemetry, DetailType::Update);
            entry.data = data.clone();
            entry.last_updated = Instant::now();
        } else {
            self.stats.inc(StatType::Telemetry, DetailType::Insert);
            guard.telemetries.push_back(Entry {
                endpoint,
                data: data.clone(),
                last_updated: Instant::now(),
                channel: Arc::clone(channel),
            });

            if guard.telemetries.len() > Self::MAX_SIZE {
                self.stats.inc(StatType::Telemetry, DetailType::Overfill);
                guard.telemetries.pop_front(); // Erase oldest entry
            }
        }

        drop(guard);

        (self.notify)(data, channel);

        self.stats.inc(StatType::Telemetry, DetailType::Process);
    }

    fn trigger(&self) {
        self.mutex.lock().unwrap().triggered = true;
        self.condition.notify_all();
    }

    fn len(&self) -> usize {
        self.mutex.lock().unwrap().telemetries.len()
    }

    fn request_predicate(&self, data: &TelemetryImpl) -> bool {
        if data.triggered {
            return true;
        }
        if self.config.enable_ongoing_requests {
            return data.last_request.is_none()
                || data.last_request.unwrap().elapsed()
                    >= Duration::from_millis(
                        self.network_params.network.telemetry_request_interval_ms as u64,
                    );
        }

        return false;
    }

    fn broadcast_predicate(&self, data: &TelemetryImpl) -> bool {
        if self.config.enable_ongoing_broadcasts {
            return data.last_broadcast.is_none()
                || data.last_broadcast.unwrap().elapsed()
                    >= Duration::from_millis(
                        self.network_params.network.telemetry_broadcast_interval_ms as u64,
                    );
        }

        return false;
    }

    fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            self.stats.inc(StatType::Telemetry, DetailType::Loop);

            self.cleanup();

            if self.request_predicate(&guard) {
                guard.triggered = false;
                drop(guard);

                self.run_requests();

                guard = self.mutex.lock().unwrap();
                guard.last_request = Some(Instant::now());
            }

            if self.broadcast_predicate(&guard) {
                drop(guard);

                self.run_broadcasts();

                guard = self.mutex.lock().unwrap();
                guard.last_broadcast = Some(Instant::now());
            }

            let wait_duration = min(
                self.network_params.network.telemetry_request_interval_ms,
                self.network_params.network.telemetry_broadcast_interval_ms / 2,
            );
            guard = self
                .condition
                .wait_timeout(guard, Duration::from_millis(wait_duration as u64))
                .unwrap()
                .0
        }
    }

    fn run_requests(&self) {
        let peers = self.channels.random_list(usize::MAX, 0, true);
        for channel in peers {
            self.request(&channel);
        }
    }

    fn request(&self, channel: &ChannelEnum) {
        self.stats.inc(StatType::Telemetry, DetailType::Request);
        let message = Message::TelemetryReq;
        channel.send(
            &message,
            None,
            BufferDropPolicy::Limiter,
            TrafficType::Generic,
        );
    }

    fn run_broadcasts(&self) {
        todo!();
    }

    fn cleanup(&self) {
        todo!();
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
    }
}

pub struct TelementryConfig {
    pub enable_ongoing_requests: bool,
    pub enable_ongoing_broadcasts: bool,
}

pub trait TelementryExt {
    fn start(&self);
}

struct TelemetryImpl {
    stopped: bool,
    triggered: bool,
    telemetries: OrderedTelemetries,
    last_request: Option<Instant>,
    last_broadcast: Option<Instant>,
}

impl TelementryExt for Arc<Telemetry> {
    fn start(&self) {
        debug_assert!(self.thread.lock().unwrap().is_none());
        let self_l = Arc::clone(self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Telemetry".to_string())
                .spawn(move || {
                    self_l.run();
                })
                .unwrap(),
        );
    }
}

struct Entry {
    endpoint: SocketAddrV6,
    data: TelemetryData,
    last_updated: Instant,
    channel: Arc<ChannelEnum>,
}

#[derive(Default)]
struct OrderedTelemetries {
    by_endpoint: HashMap<SocketAddrV6, Entry>,
    sequenced: VecDeque<SocketAddrV6>,
}

impl OrderedTelemetries {
    fn len(&self) -> usize {
        self.sequenced.len()
    }

    fn push_back(&mut self, entry: Entry) {
        let endpoint = entry.endpoint;
        if let Some(old) = self.by_endpoint.insert(endpoint, entry) {
            self.sequenced.retain(|i| *i != old.endpoint);
        }
        self.sequenced.push_back(endpoint);
    }

    fn get_mut(&mut self, entpoint: &SocketAddrV6) -> Option<&mut Entry> {
        self.by_endpoint.get_mut(entpoint)
    }

    fn pop_front(&mut self) {
        if let Some(endpoint) = self.sequenced.pop_front() {
            self.by_endpoint.remove(&endpoint);
        }
    }
}
