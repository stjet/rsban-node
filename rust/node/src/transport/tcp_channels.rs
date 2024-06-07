use super::{
    BufferDropPolicy, ChannelEnum, ChannelFake, ChannelTcp, ConnectionDirection, NetworkFilter,
    NullSocketObserver, OutboundBandwidthLimiter, PeerExclusion, Socket, SocketExtensions,
    SocketObserver, SynCookies, TcpListener, TcpListenerExt, TcpMessageManager, TcpServer,
    TrafficType, TransportType,
};
use crate::{
    bootstrap::ChannelEntry,
    config::{NetworkConstants, NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
    transport::Channel,
    utils::{
        ipv4_address_or_ipv6_subnet, map_address_to_subnetwork, reserved_address, AsyncRuntime,
        ThreadPool, ThreadPoolImpl,
    },
    NetworkParams, DEV_NETWORK_PARAMS,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent, OutputListenerMt, OutputTrackerMt},
    Account, KeyPair, PublicKey,
};
use rsnano_messages::*;
use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
    mem::size_of,
    net::{Ipv6Addr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    time::SystemTime,
};
use tracing::{debug, warn};

pub struct TcpChannelsOptions {
    pub node_config: NodeConfig,
    pub publish_filter: Arc<NetworkFilter>,
    pub async_rt: Arc<AsyncRuntime>,
    pub network: NetworkParams,
    pub stats: Arc<Stats>,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    pub port: u16,
    pub flags: NodeFlags,
    pub limiter: Arc<OutboundBandwidthLimiter>,
    pub node_id: KeyPair,
    pub syn_cookies: Arc<SynCookies>,
    pub workers: Arc<dyn ThreadPool>,
    pub observer: Arc<dyn SocketObserver>,
}

impl TcpChannelsOptions {
    pub fn new_test_instance() -> Self {
        TcpChannelsOptions {
            node_config: NodeConfig::new_null(),
            publish_filter: Arc::new(NetworkFilter::default()),
            async_rt: Arc::new(AsyncRuntime::default()),
            network: DEV_NETWORK_PARAMS.clone(),
            stats: Arc::new(Default::default()),
            tcp_message_manager: Arc::new(TcpMessageManager::default()),
            port: 8088,
            flags: NodeFlags::default(),
            limiter: Arc::new(OutboundBandwidthLimiter::default()),
            node_id: KeyPair::new(),
            syn_cookies: Arc::new(SynCookies::default()),
            workers: Arc::new(ThreadPoolImpl::new_test_instance()),
            observer: Arc::new(NullSocketObserver::new()),
        }
    }
}

pub struct TcpChannels {
    tcp_channels: Mutex<TcpChannelsImpl>,
    tcp_listener: RwLock<Option<Weak<TcpListener>>>,
    port: AtomicU16,
    stopped: AtomicBool,
    allow_local_peers: bool,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    flags: NodeFlags,
    stats: Arc<Stats>,
    sink: RwLock<Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>>,
    next_channel_id: AtomicUsize,
    network: Arc<NetworkParams>,
    pub excluded_peers: Arc<Mutex<PeerExclusion>>,
    limiter: Arc<OutboundBandwidthLimiter>,
    async_rt: Arc<AsyncRuntime>,
    node_config: Arc<NodeConfig>,
    node_id: KeyPair,
    syn_cookies: Arc<SynCookies>,
    workers: Arc<dyn ThreadPool>,
    pub publish_filter: Arc<NetworkFilter>,
    observer: Arc<dyn SocketObserver>,
    merge_peer_listener: OutputListenerMt<SocketAddrV6>,
}

impl Drop for TcpChannels {
    fn drop(&mut self) {
        self.stop();
    }
}

impl TcpChannels {
    pub fn new(options: TcpChannelsOptions) -> Self {
        let node_config = Arc::new(options.node_config);
        let network = Arc::new(options.network);

        Self {
            tcp_listener: RwLock::new(None),
            port: AtomicU16::new(options.port),
            stopped: AtomicBool::new(false),
            allow_local_peers: node_config.allow_local_peers,
            node_config,
            tcp_message_manager: options.tcp_message_manager.clone(),
            flags: options.flags,
            stats: options.stats,
            tcp_channels: Mutex::new(TcpChannelsImpl {
                attempts: Default::default(),
                channels: Default::default(),
                network_constants: network.network.clone(),
                new_channel_observers: Vec::new(),
            }),
            sink: RwLock::new(Box::new(|_, _| {})),
            next_channel_id: AtomicUsize::new(1),
            network,
            excluded_peers: Arc::new(Mutex::new(PeerExclusion::new())),
            limiter: options.limiter,
            node_id: options.node_id,
            syn_cookies: options.syn_cookies,
            workers: options.workers,
            publish_filter: options.publish_filter,
            observer: options.observer,
            async_rt: options.async_rt,
            merge_peer_listener: OutputListenerMt::new(),
        }
    }

    pub fn set_listener(&self, listener: Weak<TcpListener>) {
        *self.tcp_listener.write().unwrap() = Some(listener);
    }

    pub fn set_sink(&self, sink: Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>) {
        *self.sink.write().unwrap() = sink;
    }

    pub fn new_null() -> Self {
        Self::new(TcpChannelsOptions::new_test_instance())
    }

    pub fn get_next_channel_id(&self) -> usize {
        self.next_channel_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.tcp_message_manager.stop();
            self.close();
        }
    }

    fn close(&self) {
        self.tcp_channels.lock().unwrap().close_channels();
    }

    pub fn count_per_direction(&self, direction: ConnectionDirection) -> usize {
        self.tcp_channels
            .lock()
            .unwrap()
            .channels
            .iter()
            .filter(|entry| entry.channel.direction() == direction)
            .count()
    }

    pub fn count_per_ip(&self, ip: &Ipv6Addr) -> usize {
        self.tcp_channels
            .lock()
            .unwrap()
            .channels
            .iter()
            .filter(|entry| entry.channel.remote_endpoint().ip() == ip)
            .count()
    }

    pub fn count_per_subnetwork(&self, ip: &Ipv6Addr) -> usize {
        let subnet = map_address_to_subnetwork(ip);

        self.tcp_channels
            .lock()
            .unwrap()
            .channels
            .iter()
            .filter(|entry| {
                map_address_to_subnetwork(entry.channel.remote_endpoint().ip()) == subnet
            })
            .count()
    }

    pub fn not_a_peer(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        endpoint.ip().is_unspecified()
            || reserved_address(endpoint, allow_local_peers)
            || endpoint
                == &SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port.load(Ordering::SeqCst), 0, 0)
    }

    pub fn on_new_channel(&self, callback: Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>) {
        self.tcp_channels
            .lock()
            .unwrap()
            .new_channel_observers
            .push(callback);
    }

    pub fn insert_fake(&self, endpoint: SocketAddrV6) {
        let fake = Arc::new(ChannelEnum::Fake(ChannelFake::new(
            SystemTime::now(),
            self.get_next_channel_id(),
            &self.async_rt,
            Arc::clone(&self.limiter),
            Arc::clone(&self.stats),
            endpoint,
            self.network.network.protocol_info(),
        )));
        fake.set_node_id(PublicKey::from(fake.channel_id() as u64));
        let mut channels = self.tcp_channels.lock().unwrap();
        channels
            .channels
            .insert(Arc::new(ChannelEntry::new(fake, None)));
    }

    fn check(
        &self,
        endpoint: &SocketAddrV6,
        node_id: &Account,
        channels: &TcpChannelsImpl,
    ) -> bool {
        if self.stopped.load(Ordering::SeqCst) {
            return false; // Reject
        }

        if self.not_a_peer(endpoint, self.node_config.allow_local_peers) {
            self.stats
                .inc(StatType::TcpChannelsRejected, DetailType::NotAPeer);
            debug!("Rejected invalid endpoint channel from: {}", endpoint);

            return false; // Reject
        }

        let has_duplicate = channels.channels.iter().any(|entry| {
            if entry.endpoint().ip() == endpoint.ip() {
                // Only counsider channels with the same node id as duplicates if they come from the same IP
                if entry.node_id() == Some(*node_id) {
                    return true;
                }
            }

            false
        });

        if has_duplicate {
            self.stats
                .inc(StatType::TcpChannelsRejected, DetailType::ChannelDuplicate);
            debug!(
                "Duplicate channel rejected from: {} ({})",
                endpoint,
                node_id.to_node_id()
            );

            return false; // Reject
        }

        true // OK
    }

    pub fn find_channel(&self, endpoint: &SocketAddrV6) -> Option<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().find_channel(endpoint)
    }

    pub fn random_channels(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels
            .lock()
            .unwrap()
            .random_channels(count, min_version)
    }

    pub fn get_peers(&self) -> Vec<SocketAddrV6> {
        self.tcp_channels.lock().unwrap().get_peers()
    }

    pub fn get_first_channel(&self) -> Option<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().get_first_channel()
    }

    pub fn find_node_id(&self, node_id: &PublicKey) -> Option<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().find_node_id(node_id)
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        self.tcp_channels
            .lock()
            .unwrap()
            .collect_container_info(name.into())
    }

    pub fn random_fill(&self, endpoints: &mut [SocketAddrV6]) {
        self.tcp_channels.lock().unwrap().random_fill(endpoints);
    }

    pub fn random_fanout(&self, scale: f32) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().random_fanout(scale)
    }

    pub fn random_list(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels
            .lock()
            .unwrap()
            .random_channels(count, min_version)
    }

    pub fn flood_message2(&self, message: &Message, drop_policy: BufferDropPolicy, scale: f32) {
        let channels = self.random_fanout(scale);
        for channel in channels {
            channel.send(message, None, drop_policy, TrafficType::Generic)
        }
    }

    pub fn flood_message(&self, message: &Message, scale: f32) {
        let channels = self.random_fanout(scale);
        for channel in channels {
            channel.send(
                message,
                None,
                BufferDropPolicy::Limiter,
                TrafficType::Generic,
            )
        }
    }

    pub fn max_ip_or_subnetwork_connections(&self, endpoint: &SocketAddrV6) -> bool {
        self.max_ip_connections(endpoint) || self.max_subnetwork_connections(endpoint)
    }

    pub fn max_ip_connections(&self, endpoint: &SocketAddrV6) -> bool {
        if self.flags.disable_max_peers_per_ip {
            return false;
        }
        let mut result;
        let address = ipv4_address_or_ipv6_subnet(endpoint.ip());
        let lock = self.tcp_channels.lock().unwrap();
        result = lock.channels.count_by_ip(&address) >= lock.network_constants.max_peers_per_ip;
        if !result {
            result =
                lock.attempts.count_by_address(&address) >= lock.network_constants.max_peers_per_ip;
        }
        if result {
            self.stats
                .inc_dir(StatType::Tcp, DetailType::MaxPerIp, Direction::Out);
        }
        result
    }

    pub fn verify_handshake_response(
        &self,
        response: &NodeIdHandshakeResponse,
        remote_endpoint: SocketAddrV6,
    ) -> bool {
        // Prevent connection with ourselves
        if response.node_id == self.node_id.public_key() {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::InvalidNodeId,
                Direction::In,
            );
            return false; // Fail
        }

        // Prevent mismatched genesis
        if let Some(v2) = &response.v2 {
            if v2.genesis != self.network.ledger.genesis.hash() {
                self.stats.inc_dir(
                    StatType::Handshake,
                    DetailType::InvalidGenesis,
                    Direction::In,
                );
                return false; // Fail
            }
        }

        let Some(cookie) = self.syn_cookies.cookie(&remote_endpoint) else {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::MissingCookie,
                Direction::In,
            );
            return false; // Fail
        };

        if response.validate(&cookie).is_err() {
            self.stats.inc_dir(
                StatType::Handshake,
                DetailType::InvalidSignature,
                Direction::In,
            );
            return false; // Fail
        }

        self.stats
            .inc_dir(StatType::Handshake, DetailType::Ok, Direction::In);
        true
    }

    pub fn prepare_handshake_response(
        &self,
        query_payload: &NodeIdHandshakeQuery,
        v2: bool,
    ) -> NodeIdHandshakeResponse {
        if v2 {
            let genesis = self.network.ledger.genesis.hash();
            NodeIdHandshakeResponse::new_v2(&query_payload.cookie, &self.node_id, genesis)
        } else {
            NodeIdHandshakeResponse::new_v1(&query_payload.cookie, &self.node_id)
        }
    }

    pub fn prepare_handshake_query(
        &self,
        remote_endpoint: SocketAddrV6,
    ) -> Option<NodeIdHandshakeQuery> {
        self.syn_cookies
            .assign(&remote_endpoint)
            .map(|cookie| NodeIdHandshakeQuery { cookie })
    }

    pub fn max_subnetwork_connections(&self, endoint: &SocketAddrV6) -> bool {
        if self.flags.disable_max_peers_per_subnetwork {
            return false;
        }

        let subnet = map_address_to_subnetwork(endoint.ip());
        let guard = self.tcp_channels.lock().unwrap();

        let is_max = guard.channels.count_by_subnet(&subnet)
            >= self.network.network.max_peers_per_subnetwork
            || guard.attempts.count_by_subnetwork(&subnet)
                >= self.network.network.max_peers_per_subnetwork;

        if is_max {
            self.stats
                .inc_dir(StatType::Tcp, DetailType::MaxPerSubnetwork, Direction::Out);
        }

        is_max
    }

    pub fn reachout_checked(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        // Don't contact invalid IPs
        let mut error = self.not_a_peer(endpoint, allow_local_peers);
        if !error {
            error = !self.track_reachout(endpoint);
        }
        error
    }

    pub fn track_reachout(&self, endpoint: &SocketAddrV6) -> bool {
        // Don't overload single IP
        if self.max_ip_or_subnetwork_connections(endpoint) {
            return false;
        }
        if self.excluded_peers.lock().unwrap().is_excluded(endpoint) {
            return false;
        }
        if self.flags.disable_tcp_realtime {
            return false;
        }
        // Don't connect to nodes that already sent us something
        if self.find_channel(endpoint).is_some() {
            return false;
        }

        let mut guard = self.tcp_channels.lock().unwrap();
        let attempt = AttemptEntry::new(*endpoint);
        let inserted = guard.attempts.insert(attempt);
        inserted
    }

    pub fn len_sqrt(&self) -> f32 {
        self.tcp_channels.lock().unwrap().len_sqrt()
    }
    /// Desired fanout for a given scale
    /// Simulating with sqrt_broadcast_simulate shows we only need to broadcast to sqrt(total_peers) random peers in order to successfully publish to everyone with high probability
    pub fn fanout(&self, scale: f32) -> usize {
        self.tcp_channels.lock().unwrap().fanout(scale)
    }

    pub fn purge(&self, cutoff: SystemTime) {
        let mut guard = self.tcp_channels.lock().unwrap();
        guard.purge(cutoff);
    }

    pub fn erase_channel_by_endpoint(&self, endpoint: &SocketAddrV6) {
        self.tcp_channels
            .lock()
            .unwrap()
            .channels
            .remove_by_endpoint(endpoint);
    }

    pub fn len(&self) -> usize {
        self.tcp_channels.lock().unwrap().channels.len()
    }

    pub fn bootstrap_peer(&self) -> SocketAddrV6 {
        self.tcp_channels.lock().unwrap().bootstrap_peer()
    }

    pub fn list_channels(&self, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        let mut result = self.tcp_channels.lock().unwrap().list(min_version);
        result.sort_by_key(|i| i.remote_endpoint());
        result
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::SeqCst);
    }

    pub fn create_keepalive_message(&self) -> Message {
        let mut peers = [SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0); 8];
        self.random_fill(&mut peers);
        Message::Keepalive(Keepalive { peers })
    }

    pub fn sample_keepalive(&self) -> Option<Keepalive> {
        let channels = self.tcp_channels.lock().unwrap();
        let mut rng = thread_rng();
        for _ in 0..channels.channels.len() {
            let index = rng.gen_range(0..channels.channels.len());
            if let Some(channel) = channels.channels.get_by_index(index) {
                if let Some(server) = &channel.response_server {
                    if let Some(keepalive) = server.pop_last_keepalive() {
                        return Some(keepalive);
                    }
                }
            }
        }

        None
    }

    pub fn exclude(&self, channel: &Arc<ChannelEnum>) {
        // Add to peer exclusion list
        self.excluded_peers
            .lock()
            .unwrap()
            .peer_misbehaved(&channel.remote_endpoint());

        // Disconnect
        if channel.get_type() == TransportType::Tcp {
            self.erase_channel_by_endpoint(&channel.remote_endpoint())
        }
    }

    pub fn track_merge_peer(&self) -> Arc<OutputTrackerMt<SocketAddrV6>> {
        self.merge_peer_listener.track()
    }

    pub fn queue_message(&self, message: DeserializedMessage, channel: Arc<ChannelEnum>) {
        if !self.stopped.load(Ordering::SeqCst) {
            self.tcp_message_manager.put(message, channel);
        }
    }
}

pub trait TcpChannelsExtension {
    fn create(
        &self,
        socket: Arc<Socket>,
        server: Arc<TcpServer>,
        node_id: Account,
    ) -> Option<Arc<ChannelEnum>>;

    fn process_messages(&self);
    fn merge_peer(&self, peer: SocketAddrV6);
    fn keepalive(&self);
    fn connect(&self, endpoint: SocketAddrV6);
}

impl TcpChannelsExtension for Arc<TcpChannels> {
    // This should be the only place in node where channels are created
    fn create(
        &self,
        socket: Arc<Socket>,
        server: Arc<TcpServer>,
        node_id: Account,
    ) -> Option<Arc<ChannelEnum>> {
        let endpoint = socket.get_remote().unwrap();

        let mut lock = self.tcp_channels.lock().unwrap();

        if self.stopped.load(Ordering::SeqCst) {
            return None;
        }

        if !self.check(&endpoint, &node_id, &lock) {
            self.stats
                .inc(StatType::TcpChannels, DetailType::ChannelRejected);
            debug!(
                "Rejected new channel from: {} ({})",
                endpoint,
                node_id.to_node_id()
            );
            // Rejection reason should be logged earlier

            return None;
        }

        self.stats
            .inc(StatType::TcpChannels, DetailType::ChannelAccepted);
        debug!(
            "Accepted new channel from: {} ({})",
            endpoint,
            node_id.to_node_id()
        );

        let tcp_channel = ChannelTcp::new(
            socket,
            SystemTime::now(),
            Arc::clone(&self.stats),
            self,
            Arc::clone(&self.limiter),
            &self.async_rt,
            self.get_next_channel_id(),
            self.network.network.protocol_info(),
        );
        tcp_channel.update_remote_endpoint();
        let channel = Arc::new(ChannelEnum::Tcp(Arc::new(tcp_channel)));
        channel.set_node_id(node_id);

        lock.attempts.remove(&endpoint);

        let inserted = lock.channels.insert(Arc::new(ChannelEntry::new(
            Arc::clone(&channel),
            Some(server),
        )));
        debug_assert!(inserted);

        let observers = lock.new_channel_observers.clone();
        drop(lock);

        for observer in observers {
            observer(channel.clone());
        }

        Some(channel)
    }

    fn process_messages(&self) {
        while !self.stopped.load(Ordering::SeqCst) {
            if let Some((message, channel)) = self.tcp_message_manager.next() {
                (self.sink.read().unwrap())(message, channel)
            }
        }
    }

    fn merge_peer(&self, peer: SocketAddrV6) {
        self.merge_peer_listener.emit(peer);
        if !self.reachout_checked(&peer, self.node_config.allow_local_peers) {
            self.stats.inc(StatType::Network, DetailType::MergePeer);
            self.connect(peer);
        }
    }

    fn keepalive(&self) {
        let message = self.create_keepalive_message();

        // Wake up channels
        let to_wake_up = {
            let guard = self.tcp_channels.lock().unwrap();
            guard.keepalive_list()
        };

        for channel in to_wake_up {
            let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
                continue;
            };
            tcp.send(
                &message,
                None,
                BufferDropPolicy::Limiter,
                TrafficType::Generic,
            );
        }
    }

    fn connect(&self, endpoint: SocketAddrV6) {
        let listener = {
            let guard = self.tcp_listener.read().unwrap();

            let Some(listener) = guard.as_ref() else {
                warn!("Tcp listener not set!");
                return;
            };

            let Some(listener) = listener.upgrade() else {
                warn!("Tcp listener already dropped!");
                return;
            };
            listener
        };

        listener.connect(endpoint);
    }
}

pub struct TcpChannelsImpl {
    pub attempts: TcpEndpointAttemptContainer,
    pub channels: ChannelContainer,
    network_constants: NetworkConstants,
    new_channel_observers: Vec<Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>>,
}

impl TcpChannelsImpl {
    pub fn bootstrap_peer(&mut self) -> SocketAddrV6 {
        let mut channel_endpoint = None;
        let mut peering_endpoint = None;
        for channel in self.channels.iter_by_last_bootstrap_attempt() {
            if channel.network_version() >= self.network_constants.protocol_version_min {
                if let ChannelEnum::Tcp(tcp) = channel.channel.as_ref() {
                    channel_endpoint = Some(channel.endpoint());
                    peering_endpoint = Some(tcp.peering_endpoint());
                    break;
                }
            }
        }

        match (channel_endpoint, peering_endpoint) {
            (Some(ep), Some(peering)) => {
                self.channels
                    .set_last_bootstrap_attempt(&ep, SystemTime::now());
                peering
            }
            _ => SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0),
        }
    }

    pub fn close_channels(&mut self) {
        for channel in self.channels.iter() {
            channel.close_socket();
            // Remove response server
            if let Some(server) = &channel.response_server {
                server.stop();
            }
        }
        self.channels.clear();
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        self.channels.close_idle_channels(cutoff);

        // Check if any tcp channels belonging to old protocol versions which may still be alive due to async operations
        self.channels
            .close_old_protocol_versions(self.network_constants.protocol_version_min);

        // Remove channels with dead underlying sockets
        self.channels.remove_dead();

        // Remove keepalive attempt tracking for attempts older than cutoff
        self.attempts.purge(cutoff);
    }

    pub fn random_channels(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        let mut channels = self.list(min_version);
        let mut rng = thread_rng();
        channels.shuffle(&mut rng);
        if count > 0 {
            channels.truncate(count)
        }
        channels
    }

    pub fn list(&self, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        self.channels
            .iter()
            .filter(|c| c.network_version() >= min_version && c.channel.is_alive())
            .map(|c| c.channel.clone())
            .collect()
    }

    pub fn keepalive_list(&self) -> Vec<Arc<ChannelEnum>> {
        let cutoff = SystemTime::now() - self.network_constants.keepalive_period;
        let mut result = Vec::new();
        for channel in self.channels.iter() {
            if channel.last_packet_sent() < cutoff {
                result.push(channel.channel.clone());
            }
        }

        result
    }

    pub fn find_channel(&self, endpoint: &SocketAddrV6) -> Option<Arc<ChannelEnum>> {
        self.channels.get(endpoint).map(|c| c.channel.clone())
    }

    pub fn get_peers(&self) -> Vec<SocketAddrV6> {
        // We can't hold the mutex while starting a write transaction, so
        // we collect endpoints to be saved and then release the lock.
        self.channels.iter().map(|c| c.endpoint()).collect()
    }

    pub fn get_first_channel(&self) -> Option<Arc<ChannelEnum>> {
        self.channels.get_by_index(0).map(|c| c.channel.clone())
    }

    pub fn find_node_id(&self, node_id: &PublicKey) -> Option<Arc<ChannelEnum>> {
        self.channels
            .get_by_node_id(node_id)
            .map(|c| c.channel.clone())
    }

    pub fn random_fanout(&self, scale: f32) -> Vec<Arc<ChannelEnum>> {
        self.random_channels(self.fanout(scale), 0)
    }

    pub fn random_fill(&self, endpoints: &mut [SocketAddrV6]) {
        // Don't include channels with ephemeral remote ports
        let peers = self.random_channels(endpoints.len(), 0);
        let null_endpoint = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);
        for (i, target) in endpoints.iter_mut().enumerate() {
            let endpoint = if i < peers.len() {
                let ChannelEnum::Tcp(tcp) = peers[i].as_ref() else {
                    panic!("not a tcp channel")
                };
                tcp.peering_endpoint()
            } else {
                null_endpoint
            };
            *target = endpoint;
        }
    }

    pub fn len_sqrt(&self) -> f32 {
        f32::sqrt(self.channels.len() as f32)
    }

    pub fn fanout(&self, scale: f32) -> usize {
        (self.len_sqrt() * scale).ceil() as usize
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name,
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "channels".to_string(),
                    count: self.channels.len(),
                    sizeof_element: size_of::<ChannelEntry>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "attempts".to_string(),
                    count: self.attempts.len(),
                    sizeof_element: size_of::<AttemptEntry>(),
                }),
            ],
        )
    }
}

#[derive(Default)]
pub struct ChannelContainer {
    by_endpoint: HashMap<SocketAddrV6, Arc<ChannelEntry>>,
    by_random_access: Vec<SocketAddrV6>,
    by_bootstrap_attempt: BTreeMap<SystemTime, Vec<SocketAddrV6>>,
    by_node_id: HashMap<PublicKey, Vec<SocketAddrV6>>,
    by_network_version: BTreeMap<u8, Vec<SocketAddrV6>>,
    by_ip_address: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
    by_subnet: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
}

impl ChannelContainer {
    pub fn insert(&mut self, wrapper: Arc<ChannelEntry>) -> bool {
        let endpoint = wrapper.endpoint();
        if self.by_endpoint.contains_key(&endpoint) {
            return false;
        }

        self.by_random_access.push(endpoint);
        self.by_bootstrap_attempt
            .entry(wrapper.last_bootstrap_attempt())
            .or_default()
            .push(endpoint);
        self.by_node_id
            .entry(wrapper.node_id().unwrap_or_default())
            .or_default()
            .push(endpoint);
        self.by_network_version
            .entry(wrapper.network_version())
            .or_default()
            .push(endpoint);
        self.by_ip_address
            .entry(wrapper.ip_address())
            .or_default()
            .push(endpoint);
        self.by_subnet
            .entry(wrapper.subnetwork())
            .or_default()
            .push(endpoint);
        self.by_endpoint.insert(wrapper.endpoint(), wrapper);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<ChannelEntry>> {
        self.by_endpoint.values()
    }

    pub fn iter_by_last_bootstrap_attempt(&self) -> impl Iterator<Item = &Arc<ChannelEntry>> {
        self.by_bootstrap_attempt
            .iter()
            .flat_map(|(_, v)| v.iter().map(|ep| self.by_endpoint.get(ep).unwrap()))
    }

    pub fn exists(&self, endpoint: &SocketAddrV6) -> bool {
        self.by_endpoint.contains_key(endpoint)
    }

    pub fn remove_by_node_id(&mut self, node_id: &PublicKey) {
        if let Some(endpoints) = self.by_node_id.get(node_id).cloned() {
            for ep in endpoints {
                self.remove_by_endpoint(&ep);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }

    pub fn remove_by_endpoint(&mut self, endpoint: &SocketAddrV6) -> Option<Arc<ChannelEnum>> {
        if let Some(entry) = self.by_endpoint.remove(endpoint) {
            self.by_random_access.retain(|x| x != endpoint); // todo: linear search is slow?

            remove_endpoint_btree(
                &mut self.by_bootstrap_attempt,
                &entry.last_bootstrap_attempt(),
                endpoint,
            );
            remove_endpoint_map(
                &mut self.by_node_id,
                &entry.node_id().unwrap_or_default(),
                endpoint,
            );
            remove_endpoint_btree(
                &mut self.by_network_version,
                &entry.network_version(),
                endpoint,
            );
            remove_endpoint_map(&mut self.by_ip_address, &entry.ip_address(), endpoint);
            remove_endpoint_map(&mut self.by_subnet, &entry.subnetwork(), endpoint);
            Some(entry.channel.clone())
        } else {
            None
        }
    }

    pub fn get(&self, endpoint: &SocketAddrV6) -> Option<&Arc<ChannelEntry>> {
        self.by_endpoint.get(endpoint)
    }

    pub fn get_by_index(&self, index: usize) -> Option<&Arc<ChannelEntry>> {
        self.by_random_access
            .get(index)
            .map(|ep| self.by_endpoint.get(ep))
            .flatten()
    }

    pub fn get_by_node_id(&self, node_id: &PublicKey) -> Option<&Arc<ChannelEntry>> {
        self.by_node_id
            .get(node_id)
            .map(|endpoints| self.by_endpoint.get(&endpoints[0]))
            .flatten()
    }

    pub fn set_last_bootstrap_attempt(
        &mut self,
        endpoint: &SocketAddrV6,
        attempt_time: SystemTime,
    ) {
        if let Some(channel) = self.by_endpoint.get(endpoint) {
            let old_time = channel.last_bootstrap_attempt();
            channel.channel.set_last_bootstrap_attempt(attempt_time);
            remove_endpoint_btree(
                &mut self.by_bootstrap_attempt,
                &old_time,
                &channel.endpoint(),
            );
            self.by_bootstrap_attempt
                .entry(attempt_time)
                .or_default()
                .push(*endpoint);
        }
    }

    pub fn count_by_ip(&self, ip: &Ipv6Addr) -> usize {
        self.by_ip_address
            .get(ip)
            .map(|endpoints| endpoints.len())
            .unwrap_or_default()
    }

    pub fn count_by_subnet(&self, subnet: &Ipv6Addr) -> usize {
        self.by_subnet
            .get(subnet)
            .map(|endpoints| endpoints.len())
            .unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.by_endpoint.clear();
        self.by_random_access.clear();
        self.by_bootstrap_attempt.clear();
        self.by_node_id.clear();
        self.by_network_version.clear();
        self.by_ip_address.clear();
        self.by_subnet.clear();
    }

    pub fn close_idle_channels(&mut self, cutoff: SystemTime) {
        for entry in self.iter() {
            if entry.channel.get_last_packet_sent() < cutoff {
                debug!("Closing idle channel: {}", entry.channel.remote_endpoint());
                entry.channel.close();
            }
        }
    }

    pub fn remove_dead(&mut self) {
        let dead_channels: Vec<_> = self
            .by_endpoint
            .values()
            .filter(|c| !c.channel.is_alive())
            .cloned()
            .collect();

        for channel in dead_channels {
            debug!("Removing dead channel: {}", channel.endpoint());
            self.remove_by_endpoint(&channel.endpoint());
        }
    }

    pub fn close_old_protocol_versions(&mut self, min_version: u8) {
        while let Some((version, endpoints)) = self.by_network_version.first_key_value() {
            if *version < min_version {
                for ep in endpoints {
                    debug!(
                        "Closing channel with old protocol version: {} (channels version: {}, min version: {})",
                        ep, version, min_version
                    );
                    if let Some(entry) = self.by_endpoint.get(ep) {
                        entry.channel.close();
                    }
                }
            } else {
                break;
            }
        }
    }
}

fn remove_endpoint_btree<K: Ord>(
    tree: &mut BTreeMap<K, Vec<SocketAddrV6>>,
    key: &K,
    endpoint: &SocketAddrV6,
) {
    let endpoints = tree.get_mut(key).unwrap();
    if endpoints.len() > 1 {
        endpoints.retain(|x| x != endpoint);
    } else {
        tree.remove(key);
    }
}

fn remove_endpoint_map<K: Eq + PartialEq + Hash>(
    map: &mut HashMap<K, Vec<SocketAddrV6>>,
    key: &K,
    endpoint: &SocketAddrV6,
) {
    let endpoints = map.get_mut(key).unwrap();
    if endpoints.len() > 1 {
        endpoints.retain(|x| x != endpoint);
    } else {
        map.remove(key);
    }
}

pub struct AttemptEntry {
    pub endpoint: SocketAddrV6,
    pub address: Ipv6Addr,
    pub subnetwork: Ipv6Addr,
    pub start: SystemTime,
}

impl AttemptEntry {
    pub fn new(endpoint: SocketAddrV6) -> Self {
        Self {
            endpoint,
            address: ipv4_address_or_ipv6_subnet(endpoint.ip()),
            subnetwork: map_address_to_subnetwork(endpoint.ip()),
            start: SystemTime::now(),
        }
    }
}

#[derive(Default)]
pub struct TcpEndpointAttemptContainer {
    by_endpoint: HashMap<SocketAddrV6, AttemptEntry>,
    by_address: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
    by_subnetwork: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
    by_time: BTreeMap<SystemTime, Vec<SocketAddrV6>>,
}

impl TcpEndpointAttemptContainer {
    pub fn insert(&mut self, attempt: AttemptEntry) -> bool {
        if self.by_endpoint.contains_key(&attempt.endpoint) {
            return false;
        }
        self.by_address
            .entry(attempt.address)
            .or_default()
            .push(attempt.endpoint);
        self.by_subnetwork
            .entry(attempt.subnetwork)
            .or_default()
            .push(attempt.endpoint);
        self.by_time
            .entry(attempt.start)
            .or_default()
            .push(attempt.endpoint);
        self.by_endpoint.insert(attempt.endpoint, attempt);
        true
    }

    pub fn remove(&mut self, endpoint: &SocketAddrV6) {
        if let Some(attempt) = self.by_endpoint.remove(endpoint) {
            let by_address = self.by_address.get_mut(&attempt.address).unwrap();
            if by_address.len() > 1 {
                by_address.retain(|x| x != endpoint);
            } else {
                self.by_address.remove(&attempt.address);
            }

            let by_subnet = self.by_subnetwork.get_mut(&attempt.subnetwork).unwrap();
            if by_subnet.len() > 1 {
                by_subnet.retain(|x| x != endpoint);
            } else {
                self.by_subnetwork.remove(&attempt.subnetwork);
            }

            let by_time = self.by_time.get_mut(&attempt.start).unwrap();
            if by_time.len() > 1 {
                by_time.retain(|x| x != endpoint);
            } else {
                self.by_time.remove(&attempt.start);
            }
        }
    }

    pub fn count_by_subnetwork(&self, subnet: &Ipv6Addr) -> usize {
        match self.by_subnetwork.get(subnet) {
            Some(entries) => entries.len(),
            None => 0,
        }
    }

    pub fn count_by_address(&self, address: &Ipv6Addr) -> usize {
        match self.by_address.get(address) {
            Some(entries) => entries.len(),
            None => 0,
        }
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        while let Some((time, endpoint)) = self.get_oldest() {
            if time >= cutoff {
                return;
            }

            self.remove(&endpoint);
        }
    }

    fn get_oldest(&self) -> Option<(SystemTime, SocketAddrV6)> {
        let (time, endpoints) = self.by_time.first_key_value()?;
        Some((*time, endpoints[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::TEST_ENDPOINT_1;

    #[test]
    fn track_merge_peer() {
        let channels = Arc::new(TcpChannels::new_null());
        let merge_tracker = channels.track_merge_peer();

        channels.merge_peer(TEST_ENDPOINT_1);

        assert_eq!(merge_tracker.output(), vec![TEST_ENDPOINT_1]);
    }
}
