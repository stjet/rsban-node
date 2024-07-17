use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockHash, KeyPair, PublicKey, Signature, WorkVersion,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, TelemetryAck, TelemetryData, TelemetryMaker};
use std::{
    cmp::min,
    collections::{HashMap, VecDeque},
    mem::size_of,
    net::SocketAddrV6,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime},
};

use crate::{
    block_processing::UncheckedMap,
    config::NodeConfig,
    stats::{DetailType, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, ChannelMode, Network, TrafficType},
    NetworkParams, DEV_NETWORK_PARAMS,
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
    node_config: NodeConfig,
    stats: Arc<Stats>,
    ledger: Arc<Ledger>,
    unchecked: Arc<UncheckedMap>,
    thread: Mutex<Option<JoinHandle<()>>>,
    condition: Condvar,
    mutex: Mutex<TelemetryImpl>,
    network_params: NetworkParams,
    network: Arc<Network>,
    node_id: KeyPair,
    startup_time: Instant,
    notify: Mutex<Vec<Box<dyn Fn(&TelemetryData, &Arc<ChannelEnum>) + Send + Sync>>>,
}

impl Telemetry {
    const MAX_SIZE: usize = 1024;

    pub fn new_null() -> Self {
        Self {
            config: TelementryConfig::default(),
            node_config: NodeConfig::new_test_instance(),
            stats: Arc::new(Stats::default()),
            ledger: Arc::new(Ledger::new_null()),
            unchecked: Arc::new(UncheckedMap::default()),
            thread: Mutex::new(None),
            condition: Condvar::default(),
            mutex: Mutex::new(TelemetryImpl::default()),
            network_params: DEV_NETWORK_PARAMS.to_owned(),
            network: Arc::new(Network::new_null()),
            node_id: KeyPair::new(),
            startup_time: Instant::now(),
            notify: Mutex::new(vec![]),
        }
    }

    pub fn new(
        config: TelementryConfig,
        node_config: NodeConfig,
        stats: Arc<Stats>,
        ledger: Arc<Ledger>,
        unchecked: Arc<UncheckedMap>,
        network_params: NetworkParams,
        network: Arc<Network>,
        node_id: KeyPair,
    ) -> Self {
        Self {
            config,
            node_config,
            stats,
            ledger,
            unchecked,
            network_params,
            network,
            thread: Mutex::new(None),
            condition: Condvar::new(),
            mutex: Mutex::new(TelemetryImpl {
                stopped: false,
                triggered: false,
                telemetries: Default::default(),
                last_broadcast: None,
                last_request: None,
            }),
            notify: Mutex::new(Vec::new()),
            node_id,
            startup_time: Instant::now(),
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }

    pub fn add_callback(&self, f: Box<dyn Fn(&TelemetryData, &Arc<ChannelEnum>) + Send + Sync>) {
        self.notify.lock().unwrap().push(f);
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
            self.network.peer_misbehaved(channel);

            self.stats
                .inc(StatType::Telemetry, DetailType::GenesisMismatch);
            return false;
        }

        return true; // Telemetry is OK
    }

    /// Process telemetry message from network
    pub fn process(&self, telemetry: &TelemetryAck, channel: &Arc<ChannelEnum>) {
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

        {
            let callbacks = self.notify.lock().unwrap();
            for callback in callbacks.iter() {
                (callback)(data, channel);
            }
        }

        self.stats.inc(StatType::Telemetry, DetailType::Process);
    }

    /// Trigger manual telemetry request to all peers
    pub fn trigger(&self) {
        self.mutex.lock().unwrap().triggered = true;
        self.condition.notify_all();
    }

    pub fn len(&self) -> usize {
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

            self.cleanup(&mut guard);

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
        let peers = self.network.random_list(usize::MAX, 0);
        for channel in peers {
            self.request(&channel);
        }
    }

    fn run_broadcasts(&self) {
        let telemetry = self.local_telemetry();
        let peers = self.network.random_list(usize::MAX, 0);
        let message = Message::TelemetryAck(TelemetryAck(Some(telemetry)));
        for channel in peers {
            self.broadcast(&channel, &message);
        }
    }

    fn broadcast(&self, channel: &ChannelEnum, message: &Message) {
        self.stats.inc(StatType::Telemetry, DetailType::Broadcast);
        channel.send(
            message,
            None,
            BufferDropPolicy::Limiter,
            TrafficType::Generic,
        )
    }

    fn cleanup(&self, data: &mut TelemetryImpl) {
        data.telemetries.retain(|entry| {
            // Remove if telemetry data is stale
            if self.has_timed_out(entry) {
                self.stats
                    .inc(StatType::Telemetry, DetailType::CleanupOutdated);
                false // Erase
            } else {
                true // Retain
            }
        })
    }

    fn has_timed_out(&self, entry: &Entry) -> bool {
        entry.last_updated.elapsed()
            > Duration::from_millis(self.network_params.network.telemetry_cache_cutoff_ms as u64)
    }

    /// Returns telemetry for selected endpoint
    pub fn get_telemetry(&self, endpoint: &SocketAddrV6) -> Option<TelemetryData> {
        let guard = self.mutex.lock().unwrap();
        if let Some(entry) = guard.telemetries.get(endpoint) {
            if !self.has_timed_out(entry) {
                return Some(entry.data.clone());
            }
        }
        None
    }

    pub fn get_all_telemetries(&self) -> HashMap<SocketAddrV6, TelemetryData> {
        let guard = self.mutex.lock().unwrap();
        let mut result = HashMap::new();
        for entry in guard.telemetries.iter() {
            if !self.has_timed_out(entry) {
                result.insert(entry.endpoint, entry.data.clone());
            }
        }
        result
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "telemetries".to_string(),
                count: guard.telemetries.len(),
                sizeof_element: OrderedTelemetries::ELEMENT_SIZE,
            })],
        )
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

    pub fn local_telemetry(&self) -> TelemetryData {
        let mut telemetry_data = TelemetryData {
            node_id: self.node_id.public_key(),
            block_count: self.ledger.block_count(),
            cemented_count: self.ledger.cemented_count(),
            bandwidth_cap: self.node_config.bandwidth_limit as u64,
            protocol_version: self.network_params.network.protocol_version,
            uptime: self.startup_time.elapsed().as_secs(),
            unchecked_count: self.unchecked.len() as u64,
            genesis_block: self.network_params.ledger.genesis.hash(),
            peer_count: self.network.count_by_mode(ChannelMode::Realtime) as u32,
            account_count: self.ledger.account_count(),
            major_version: MAJOR_VERSION,
            minor_version: MINOR_VERSION,
            patch_version: PATCH_VERSION,
            pre_release_version: PRE_RELEASE_VERSION,
            maker: TelemetryMaker::RsNano as u8,
            timestamp: SystemTime::now(),
            active_difficulty: self.network_params.work.threshold_base(WorkVersion::Work1),
            unknown_data: Vec::new(),
            signature: Signature::default(),
        };
        // Make sure this is the final operation!
        telemetry_data.sign(&self.node_id).unwrap();
        telemetry_data
    }
}

pub const MAJOR_VERSION: u8 = 27; // TODO: get this from cmake
pub const MINOR_VERSION: u8 = 0; // TODO: get this from cmake
pub const PATCH_VERSION: u8 = 0; // TODO: get this from cmake
pub const PRE_RELEASE_VERSION: u8 = 99; // TODO: get this from cmake
pub const BUILD_INFO: &'static str = "TODO get buildinfo";
pub const VERSION_STRING: &'static str = "27.0"; // TODO: get this from cmake

#[derive(Clone, Hash, Copy, PartialEq, Eq, Default)]
struct VendorVersion {
    major: u8,
    minor: u8,
    patch: u8,
    pre_release: u8,
    maker: u8,
}

pub fn consolidate_telemetry_data(telemetry_datas: &[TelemetryData]) -> TelemetryData {
    if telemetry_datas.is_empty() {
        return Default::default();
    } else if telemetry_datas.len() == 1 {
        // Only 1 element in the collection, so just return it.
        return telemetry_datas.first().unwrap().clone();
    }

    let mut protocol_versions: HashMap<u8, i32> = HashMap::new();
    let mut vendor_versions: HashMap<VendorVersion, i32> = HashMap::new();
    let mut bandwidth_caps: HashMap<u64, i32> = HashMap::new();
    let mut genesis_blocks: HashMap<BlockHash, i32> = HashMap::new();

    // Use a trimmed average which excludes the upper and lower 10% of the results
    let mut account_counts: Vec<u64> = Vec::new();
    let mut block_counts: Vec<u64> = Vec::new();
    let mut cemented_counts: Vec<u64> = Vec::new();
    let mut peer_counts: Vec<u64> = Vec::new();
    let mut unchecked_counts: Vec<u64> = Vec::new();
    let mut uptimes: Vec<u64> = Vec::new();
    let mut bandwidths: Vec<u64> = Vec::new();
    let mut timestamps: Vec<u64> = Vec::new();
    let mut active_difficulties: Vec<u64> = Vec::new();

    for telemetry_data in telemetry_datas {
        account_counts.push(telemetry_data.account_count);
        block_counts.push(telemetry_data.block_count);
        cemented_counts.push(telemetry_data.cemented_count);

        let version = VendorVersion {
            major: telemetry_data.major_version,
            minor: telemetry_data.minor_version,
            patch: telemetry_data.patch_version,
            pre_release: telemetry_data.pre_release_version,
            maker: telemetry_data.maker,
        };
        *vendor_versions.entry(version).or_default() += 1;
        timestamps.push(
            telemetry_data
                .timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );
        *protocol_versions
            .entry(telemetry_data.protocol_version)
            .or_default() += 1;
        peer_counts.push(telemetry_data.peer_count as u64);
        unchecked_counts.push(telemetry_data.unchecked_count);
        uptimes.push(telemetry_data.uptime);
        // 0 has a special meaning (unlimited), don't include it in the average as it will be heavily skewed
        if telemetry_data.bandwidth_cap != 0 {
            bandwidths.push(telemetry_data.bandwidth_cap);
        }

        *bandwidth_caps
            .entry(telemetry_data.bandwidth_cap)
            .or_default() += 1;

        *genesis_blocks
            .entry(telemetry_data.genesis_block)
            .or_default() += 1;

        active_difficulties.push(telemetry_data.active_difficulty);
    }

    // Remove 10% of the results from the lower and upper bounds to catch any outliers. Need at least 10 responses before any are removed.
    let num_either_side_to_remove = telemetry_datas.len() / 10;

    let strip_outliers_and_sum = |counts: &mut Vec<u64>| {
        if num_either_side_to_remove * 2 >= counts.len() {
            return 0u128;
        }
        counts.sort();
        counts
            .iter()
            .skip(num_either_side_to_remove)
            .take(counts.len() - num_either_side_to_remove * 2)
            .map(|i| u128::from(*i))
            .sum()
    };

    let size = (telemetry_datas.len() - num_either_side_to_remove * 2) as u128;
    let account_sum = strip_outliers_and_sum(&mut account_counts);
    let block_sum = strip_outliers_and_sum(&mut block_counts);
    let cemented_sum = strip_outliers_and_sum(&mut cemented_counts);
    let peer_sum = strip_outliers_and_sum(&mut peer_counts);
    let unchecked_sum = strip_outliers_and_sum(&mut unchecked_counts);
    let uptime_sum = strip_outliers_and_sum(&mut uptimes);
    let bandwidth_sum = strip_outliers_and_sum(&mut bandwidths);
    let active_difficulty_sum = strip_outliers_and_sum(&mut active_difficulties);
    let timestamp_sum = strip_outliers_and_sum(&mut timestamps);
    let version = get_mode(&vendor_versions, size);

    TelemetryData {
        account_count: (account_sum / size) as u64,
        block_count: (block_sum / size) as u64,
        cemented_count: (cemented_sum / size) as u64,
        peer_count: (peer_sum / size) as u32,
        uptime: (uptime_sum / size) as u64,
        unchecked_count: (unchecked_sum / size) as u64,
        active_difficulty: (active_difficulty_sum / size) as u64,
        timestamp: SystemTime::UNIX_EPOCH + Duration::from_millis((timestamp_sum / size) as u64),
        // Use the mode of protocol version and vendor version. Also use it for bandwidth cap if there is 2 or more of the same cap.
        bandwidth_cap: get_mode_or_average(&bandwidth_caps, bandwidth_sum, size),
        protocol_version: get_mode(&protocol_versions, size),
        genesis_block: get_mode(&genesis_blocks, size),
        major_version: version.major,
        minor_version: version.minor,
        patch_version: version.patch,
        pre_release_version: version.pre_release,
        maker: version.maker,
        unknown_data: Vec::new(),
        node_id: PublicKey::zero(),
        signature: Signature::default(),
    }
}

fn get_mode_or_average(collection: &HashMap<u64, i32>, sum: u128, size: u128) -> u64 {
    let Some((key, count)) = collection.iter().max_by_key(|(_k, v)| *v) else {
        return Default::default();
    };
    if *count > 1 {
        *key
    } else {
        (sum / size) as u64
    }
}

fn get_mode<T>(collection: &HashMap<T, i32>, _size: u128) -> T
where
    T: Default + Clone,
{
    let Some((key, count)) = collection.iter().max_by_key(|(_k, v)| *v) else {
        return Default::default();
    };
    if *count > 1 {
        key.clone()
    } else {
        // Just pick the first one
        collection.iter().next().unwrap().0.clone()
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
    }
}

#[derive(Default)]
pub struct TelementryConfig {
    pub enable_ongoing_requests: bool,
    pub enable_ongoing_broadcasts: bool,
}

pub trait TelementryExt {
    fn start(&self);
}

#[derive(Default)]
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
    pub const ELEMENT_SIZE: usize = size_of::<Entry>() + size_of::<SocketAddrV6>() * 2;
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

    fn get(&self, entpoint: &SocketAddrV6) -> Option<&Entry> {
        self.by_endpoint.get(entpoint)
    }

    fn get_mut(&mut self, entpoint: &SocketAddrV6) -> Option<&mut Entry> {
        self.by_endpoint.get_mut(entpoint)
    }

    fn pop_front(&mut self) {
        if let Some(endpoint) = self.sequenced.pop_front() {
            self.by_endpoint.remove(&endpoint);
        }
    }

    fn retain(&mut self, mut f: impl FnMut(&Entry) -> bool) {
        self.by_endpoint.retain(|endpoint, entry| {
            let retain = f(entry);
            if !retain {
                self.sequenced.retain(|i| i != endpoint);
            }
            retain
        })
    }

    fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.by_endpoint.values()
    }
}
