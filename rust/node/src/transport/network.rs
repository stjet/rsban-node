use super::{
    attempt_container::AttemptContainer, channel_container::ChannelContainer, BufferDropPolicy,
    ChannelEnum, ChannelFake, ChannelTcp, ConnectionDirection, NetworkFilter, NullSocketObserver,
    OutboundBandwidthLimiter, PeerExclusion, ResponseServerImpl, Socket, SocketExtensions,
    SocketObserver, SynCookies, TcpConfig, TcpListener, TcpListenerExt, TcpMessageManager,
    TrafficType, TransportType,
};
use crate::{
    config::{NetworkConstants, NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
    transport::{Channel, ResponseServerExt},
    utils::{
        ipv4_address_or_ipv6_subnet, is_ipv4_or_v4_mapped_address, map_address_to_subnetwork,
        reserved_address, AsyncRuntime, ThreadPool, ThreadPoolImpl,
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
    net::{Ipv6Addr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    time::{Duration, SystemTime},
};
use tracing::{debug, warn};

pub struct NetworkOptions {
    pub node_config: NodeConfig,
    pub publish_filter: Arc<NetworkFilter>,
    pub async_rt: Arc<AsyncRuntime>,
    pub network_params: NetworkParams,
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

impl NetworkOptions {
    pub fn new_test_instance() -> Self {
        NetworkOptions {
            node_config: NodeConfig::new_null(),
            publish_filter: Arc::new(NetworkFilter::default()),
            async_rt: Arc::new(AsyncRuntime::default()),
            network_params: DEV_NETWORK_PARAMS.clone(),
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

pub struct Network {
    state: Mutex<State>,
    // TODO remove this back reference:
    tcp_listener: RwLock<Option<Weak<TcpListener>>>,
    port: AtomicU16,
    stopped: AtomicBool,
    allow_local_peers: bool,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    flags: NodeFlags,
    stats: Arc<Stats>,
    sink: RwLock<Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>>,
    next_channel_id: AtomicUsize,
    network_params: Arc<NetworkParams>,
    limiter: Arc<OutboundBandwidthLimiter>,
    async_rt: Arc<AsyncRuntime>,
    node_config: NodeConfig,
    node_id: KeyPair,
    syn_cookies: Arc<SynCookies>,
    workers: Arc<dyn ThreadPool>,
    pub publish_filter: Arc<NetworkFilter>,
    observer: Arc<dyn SocketObserver>,
    merge_peer_listener: OutputListenerMt<SocketAddrV6>,
}

impl Drop for Network {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Network {
    pub fn new(options: NetworkOptions) -> Self {
        let node_config = options.node_config;
        let network = Arc::new(options.network_params);

        Self {
            tcp_listener: RwLock::new(None),
            port: AtomicU16::new(options.port),
            stopped: AtomicBool::new(false),
            allow_local_peers: node_config.allow_local_peers,
            tcp_message_manager: options.tcp_message_manager.clone(),
            state: Mutex::new(State {
                attempts: Default::default(),
                channels: Default::default(),
                network_constants: network.network.clone(),
                new_channel_observers: Vec::new(),
                excluded_peers: PeerExclusion::new(),
                stats: options.stats.clone(),
                node_flags: options.flags.clone(),
                config: node_config.tcp.clone(),
            }),
            node_config,
            flags: options.flags,
            stats: options.stats,
            sink: RwLock::new(Box::new(|_, _| {})),
            next_channel_id: AtomicUsize::new(1),
            network_params: network,
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

    pub async fn accept_one(
        &self,
        socket: &Arc<Socket>,
        response_server: &Arc<ResponseServerImpl>,
        direction: ConnectionDirection,
    ) -> anyhow::Result<()> {
        let Some(remote_endpoint) = socket.get_remote() else {
            return Err(anyhow!("no remote endpoint"));
        };

        let result = self.check_limits(remote_endpoint.ip(), direction);

        if result != AcceptResult::Accepted {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::AcceptRejected,
                direction.into(),
            );
            if direction == ConnectionDirection::Outbound {
                self.stats.inc_dir(
                    StatType::TcpListener,
                    DetailType::ConnectFailure,
                    Direction::Out,
                );
            }
            debug!(
                "Rejected connection from: {} ({:?})",
                remote_endpoint, direction
            );
            // Rejection reason should be logged earlier

            if let Err(e) = socket.shutdown().await {
                self.stats.inc_dir(
                    StatType::TcpListener,
                    DetailType::CloseError,
                    direction.into(),
                );
                debug!(
                    "Error while closing socket after refusing connection: {:?} ({:?})",
                    e, direction
                )
            }
            drop(socket);
            if direction == ConnectionDirection::Inbound {
                self.stats.inc_dir(
                    StatType::TcpListener,
                    DetailType::AcceptFailure,
                    Direction::In,
                );
                // Refusal reason should be logged earlier
            }
            return Err(anyhow!("check_limits failed"));
        }

        self.stats.inc_dir(
            StatType::TcpListener,
            DetailType::AcceptSuccess,
            direction.into(),
        );

        debug!("Accepted connection: {} ({:?})", remote_endpoint, direction);

        socket.set_timeout(Duration::from_secs(
            self.network_params.network.idle_timeout_s as u64,
        ));

        socket.start();
        response_server.start();

        self.observer.socket_connected(Arc::clone(&socket));

        if direction == ConnectionDirection::Outbound {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::ConnectSuccess,
                Direction::Out,
            );
            debug!("Successfully connected to: {}", remote_endpoint);
            response_server.initiate_handshake();
        }

        Ok(())
    }

    pub fn set_listener(&self, listener: Weak<TcpListener>) {
        *self.tcp_listener.write().unwrap() = Some(listener);
    }

    pub fn set_sink(&self, sink: Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>) {
        *self.sink.write().unwrap() = sink;
    }

    pub fn new_null() -> Self {
        Self::new(NetworkOptions::new_test_instance())
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.tcp_message_manager.stop();
            self.close();
        }
    }

    fn close(&self) {
        self.state.lock().unwrap().close_channels();
    }

    pub fn get_next_channel_id(&self) -> usize {
        self.next_channel_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn not_a_peer(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        endpoint.ip().is_unspecified()
            || reserved_address(endpoint, allow_local_peers)
            || endpoint
                == &SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port.load(Ordering::SeqCst), 0, 0)
    }

    pub fn on_new_channel(&self, callback: Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>) {
        self.state
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
            self.network_params.network.protocol_info(),
        )));
        fake.set_node_id(PublicKey::from(fake.channel_id() as u64));
        let mut channels = self.state.lock().unwrap();
        channels.channels.insert(fake, None);
    }

    pub(crate) fn add_outbound_attempt(&self, remote: SocketAddrV6) -> bool {
        self.state.lock().unwrap().add_outbound_attempt(remote)
    }

    pub(crate) fn check_limits(
        &self,
        ip: &Ipv6Addr,
        direction: ConnectionDirection,
    ) -> AcceptResult {
        self.state.lock().unwrap().check_limits(ip, direction)
    }

    pub(crate) fn remove_attempt(&self, remote: &SocketAddrV6) {
        self.state.lock().unwrap().attempts.remove(&remote);
    }

    fn check(&self, endpoint: &SocketAddrV6, node_id: &Account, channels: &State) -> bool {
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
        self.state.lock().unwrap().find_channel(endpoint)
    }

    pub fn random_channels(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        self.state
            .lock()
            .unwrap()
            .random_channels(count, min_version)
    }

    pub fn get_peers(&self) -> Vec<SocketAddrV6> {
        self.state.lock().unwrap().get_peers()
    }

    pub fn get_first_channel(&self) -> Option<Arc<ChannelEnum>> {
        self.state.lock().unwrap().get_first_channel()
    }

    pub fn find_node_id(&self, node_id: &PublicKey) -> Option<Arc<ChannelEnum>> {
        self.state.lock().unwrap().find_node_id(node_id)
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        self.state.lock().unwrap().collect_container_info(name)
    }

    pub fn random_fill(&self, endpoints: &mut [SocketAddrV6]) {
        self.state.lock().unwrap().random_fill(endpoints);
    }

    pub fn random_fanout(&self, scale: f32) -> Vec<Arc<ChannelEnum>> {
        self.state.lock().unwrap().random_fanout(scale)
    }

    pub fn random_list(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        self.state
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
        let lock = self.state.lock().unwrap();
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
            if v2.genesis != self.network_params.ledger.genesis.hash() {
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
            let genesis = self.network_params.ledger.genesis.hash();
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
        let guard = self.state.lock().unwrap();

        let is_max = guard.channels.count_by_subnet(&subnet)
            >= self.network_params.network.max_peers_per_subnetwork
            || guard.attempts.count_by_subnetwork(&subnet)
                >= self.network_params.network.max_peers_per_subnetwork;

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
        let mut guard = self.state.lock().unwrap();
        if guard.excluded_peers.is_excluded(endpoint) {
            return false;
        }
        if self.flags.disable_tcp_realtime {
            return false;
        }
        // Don't connect to nodes that already sent us something
        if guard.find_channel(endpoint).is_some() {
            return false;
        }

        guard
            .attempts
            .insert(*endpoint, ConnectionDirection::Outbound)
    }

    pub fn len_sqrt(&self) -> f32 {
        self.state.lock().unwrap().len_sqrt()
    }
    /// Desired fanout for a given scale
    /// Simulating with sqrt_broadcast_simulate shows we only need to broadcast to sqrt(total_peers) random peers in order to successfully publish to everyone with high probability
    pub fn fanout(&self, scale: f32) -> usize {
        self.state.lock().unwrap().fanout(scale)
    }

    pub fn purge(&self, cutoff: SystemTime) {
        let mut guard = self.state.lock().unwrap();
        guard.purge(cutoff);
    }

    pub fn erase_channel_by_endpoint(&self, endpoint: &SocketAddrV6) {
        self.state
            .lock()
            .unwrap()
            .channels
            .remove_by_endpoint(endpoint);
    }

    pub fn len(&self) -> usize {
        self.state.lock().unwrap().channels.len()
    }

    pub fn bootstrap_peer(&self) -> SocketAddrV6 {
        self.state.lock().unwrap().bootstrap_peer()
    }

    pub fn list_channels(&self, min_version: u8) -> Vec<Arc<ChannelEnum>> {
        let mut result = self.state.lock().unwrap().list(min_version);
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
        let channels = self.state.lock().unwrap();
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

    pub fn is_excluded(&self, addr: &SocketAddrV6) -> bool {
        self.state.lock().unwrap().is_excluded(addr)
    }

    pub fn is_excluded_ip(&self, ip: &Ipv6Addr) -> bool {
        self.state.lock().unwrap().is_excluded_ip(ip)
    }

    pub fn peer_misbehaved(&self, channel: &Arc<ChannelEnum>) {
        // Add to peer exclusion list
        self.state
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

pub trait NetworkExt {
    fn create(
        &self,
        socket: Arc<Socket>,
        server: Arc<ResponseServerImpl>,
        node_id: Account,
    ) -> Option<Arc<ChannelEnum>>;

    fn process_messages(&self);
    fn merge_peer(&self, peer: SocketAddrV6);
    fn keepalive(&self);
    fn connect(&self, endpoint: SocketAddrV6);
}

impl NetworkExt for Arc<Network> {
    // This should be the only place in node where channels are created
    fn create(
        &self,
        socket: Arc<Socket>,
        server: Arc<ResponseServerImpl>,
        node_id: Account,
    ) -> Option<Arc<ChannelEnum>> {
        let endpoint = socket.get_remote().unwrap();

        let mut lock = self.state.lock().unwrap();

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
            self.network_params.network.protocol_info(),
        );
        tcp_channel.update_remote_endpoint();
        let channel = Arc::new(ChannelEnum::Tcp(Arc::new(tcp_channel)));
        channel.set_node_id(node_id);

        lock.attempts.remove(&endpoint);

        let inserted = lock.channels.insert(Arc::clone(&channel), Some(server));
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
            let guard = self.state.lock().unwrap();
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

struct State {
    attempts: AttemptContainer,
    channels: ChannelContainer,
    network_constants: NetworkConstants,
    new_channel_observers: Vec<Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>>,
    excluded_peers: PeerExclusion,
    stats: Arc<Stats>,
    node_flags: NodeFlags,
    config: TcpConfig,
}

impl State {
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

    pub fn is_excluded(&mut self, endpoint: &SocketAddrV6) -> bool {
        self.excluded_peers.is_excluded(endpoint)
    }

    pub fn is_excluded_ip(&mut self, ip: &Ipv6Addr) -> bool {
        self.excluded_peers.is_excluded_ip(ip)
    }

    pub fn peer_misbehaved(&mut self, addr: &SocketAddrV6) {
        self.excluded_peers.peer_misbehaved(addr);
    }

    pub fn add_outbound_attempt(&mut self, remote: SocketAddrV6) -> bool {
        let count = self
            .attempts
            .count_by_direction(ConnectionDirection::Outbound);
        if count > self.config.max_attempts {
            self.stats.inc_dir(
                StatType::TcpListenerRejected,
                DetailType::MaxAttempts,
                Direction::Out,
            );
            debug!(
                "Max connection attempts reached ({}), unable to initiate new connection: {}",
                count,
                remote.ip()
            );
            return false; // Rejected
        }

        let count = self.attempts.count_by_address(remote.ip());
        if count >= self.config.max_attempts_per_ip {
            self.stats.inc_dir(
                StatType::TcpListenerRejected,
                DetailType::MaxAttemptsPerIp,
                Direction::Out,
            );
            debug!(
                        "Connection attempt already in progress ({}), unable to initiate new connection: {}",
                        count, remote.ip()
                    );
            return false; // Rejected
        }

        if self.check_limits(remote.ip(), ConnectionDirection::Outbound) != AcceptResult::Accepted {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::ConnectRejected,
                Direction::Out,
            );
            // Refusal reason should be logged earlier

            return false; // Rejected
        }

        self.stats.inc_dir(
            StatType::TcpListener,
            DetailType::ConnectInitiate,
            Direction::Out,
        );
        debug!("Initiate outgoing connection to: {}", remote);

        self.attempts.insert(remote, ConnectionDirection::Inbound);
        true
    }

    pub fn check_limits(&mut self, ip: &Ipv6Addr, direction: ConnectionDirection) -> AcceptResult {
        if self.is_excluded_ip(ip) {
            self.stats.inc_dir(
                StatType::TcpListenerRejected,
                DetailType::Excluded,
                direction.into(),
            );

            debug!("Rejected connection from excluded peer: {}", ip);
            return AcceptResult::Rejected;
        }

        if !self.node_flags.disable_max_peers_per_ip {
            let count = self.channels.count_by_ip(ip);
            if count >= self.network_constants.max_peers_per_ip {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerIp,
                    direction.into(),
                );
                debug!(
                    "Max connections per IP reached ({}), unable to open new connection",
                    ip
                );
                return AcceptResult::Rejected;
            }
        }

        // If the address is IPv4 we don't check for a network limit, since its address space isn't big as IPv6/64.
        if !self.node_flags.disable_max_peers_per_subnetwork
            && !is_ipv4_or_v4_mapped_address(&(*ip).into())
        {
            let subnet = map_address_to_subnetwork(ip);
            let count = self.channels.count_by_subnet(&subnet);
            if count >= self.network_constants.max_peers_per_subnetwork {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerSubnetwork,
                    direction.into(),
                );
                debug!(
                    "Max connections per subnetwork reached ({}), unable to open new connection",
                    ip
                );
                return AcceptResult::Rejected;
            }
        }

        match direction {
            ConnectionDirection::Inbound => {
                let count = self
                    .channels
                    .count_by_direction(ConnectionDirection::Inbound);

                if count >= self.config.max_inbound_connections {
                    self.stats.inc_dir(
                        StatType::TcpListenerRejected,
                        DetailType::MaxAttempts,
                        direction.into(),
                    );
                    debug!(
                        "Max inbound connections reached ({}), unable to accept new connection: {}",
                        count, ip
                    );
                    return AcceptResult::Rejected;
                }
            }
            ConnectionDirection::Outbound => {
                let count = self
                    .channels
                    .count_by_direction(ConnectionDirection::Outbound);

                if count >= self.config.max_outbound_connections {
                    self.stats.inc_dir(
                        StatType::TcpListenerRejected,
                        DetailType::MaxAttempts,
                        direction.into(),
                    );
                    debug!(
                        "Max outbound connections reached ({}), unable to initiate new connection: {}",
                        count, ip
                    );
                    return AcceptResult::Rejected;
                }
            }
        }

        AcceptResult::Accepted
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "channels".to_string(),
                    count: self.channels.len(),
                    sizeof_element: ChannelContainer::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "attempts".to_string(),
                    count: self.attempts.len(),
                    sizeof_element: AttemptContainer::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "peers".to_string(),
                    count: self.excluded_peers.size(),
                    sizeof_element: PeerExclusion::element_size(),
                }),
            ],
        )
    }
}

#[derive(PartialEq, Eq)]
pub enum AcceptResult {
    Invalid,
    Accepted,
    Rejected,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::TEST_ENDPOINT_1;

    #[test]
    fn track_merge_peer() {
        let network = Arc::new(Network::new_null());
        let merge_tracker = network.track_merge_peer();

        network.merge_peer(TEST_ENDPOINT_1);

        assert_eq!(merge_tracker.output(), vec![TEST_ENDPOINT_1]);
    }
}
