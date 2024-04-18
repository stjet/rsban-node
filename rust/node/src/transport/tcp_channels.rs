use super::{
    BufferDropPolicy, ChannelEnum, ChannelTcp, CompositeSocketObserver, EndpointType,
    MessageDeserializer, NetworkFilter, NullSocketObserver, NullTcpServerObserver,
    OutboundBandwidthLimiter, PeerExclusion, Socket, SocketBuilder, SocketExtensions,
    SocketObserver, SynCookies, TcpMessageManager, TcpServer, TcpServerFactory, TcpServerObserver,
    TrafficType,
};
use crate::{
    bootstrap::{BootstrapMessageVisitorFactory, ChannelEntry},
    config::{NetworkConstants, NodeConfig, NodeFlags},
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    transport::{Channel, SocketType},
    utils::{
        ipv4_address_or_ipv6_subnet, map_address_to_subnetwork, reserved_address, AsyncRuntime,
        ErrorCode, ThreadPool, ThreadPoolImpl,
    },
    NetworkParams, DEV_NETWORK_PARAMS,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    KeyPair, PublicKey,
};
use rsnano_messages::*;
use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
    mem::size_of,
    net::{Ipv6Addr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, SystemTime},
};
use tokio::task::spawn_blocking;
use tracing::debug;

pub struct TcpChannelsOptions {
    pub node_config: NodeConfig,
    pub publish_filter: Arc<NetworkFilter>,
    pub async_rt: Arc<AsyncRuntime>,
    pub network: NetworkParams,
    pub stats: Arc<Stats>,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    pub port: u16,
    pub flags: NodeFlags,
    pub sink: Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Sync + Send>,
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
            sink: Box::new(|_, _| {}),
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
    port: AtomicU16,
    stopped: AtomicBool,
    allow_local_peers: bool,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    flags: NodeFlags,
    stats: Arc<Stats>,
    sink: Box<dyn Fn(DeserializedMessage, Arc<ChannelEnum>) + Send + Sync>,
    next_channel_id: AtomicUsize,
    network: Arc<NetworkParams>,
    pub excluded_peers: Arc<Mutex<PeerExclusion>>,
    limiter: Arc<OutboundBandwidthLimiter>,
    async_rt: Weak<AsyncRuntime>,
    node_config: Arc<NodeConfig>,
    node_id: KeyPair,
    syn_cookies: Arc<SynCookies>,
    workers: Arc<dyn ThreadPool>,
    pub publish_filter: Arc<NetworkFilter>,
    tcp_server_factory: Arc<Mutex<TcpServerFactory>>,
    observer: Arc<dyn SocketObserver>,
}

impl Drop for TcpChannels {
    fn drop(&mut self) {
        debug_assert_eq!(self.tcp_channels.lock().unwrap().channels.len(), 0);
        self.stop();
    }
}

impl TcpChannels {
    pub fn new(options: TcpChannelsOptions) -> Self {
        let node_config = Arc::new(options.node_config);
        let network = Arc::new(options.network);
        let observer: Arc<dyn TcpServerObserver> = Arc::new(NullTcpServerObserver {});
        let tcp_server_factory = Arc::new(Mutex::new(TcpServerFactory {
            async_rt: Arc::clone(&options.async_rt),
            config: node_config.clone(),
            observer: Arc::downgrade(&observer),
            publish_filter: options.publish_filter.clone(),
            network: network.clone(),
            stats: options.stats.clone(),
            tcp_message_manager: options.tcp_message_manager.clone(),
            message_visitor_factory: None,
            syn_cookies: Arc::clone(&options.syn_cookies),
            node_id: options.node_id.clone(),
        }));

        Self {
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
                new_channel_observer: None,
                tcp_server_factory: tcp_server_factory.clone(),
            }),
            sink: options.sink,
            next_channel_id: AtomicUsize::new(1),
            network,
            excluded_peers: Arc::new(Mutex::new(PeerExclusion::new())),
            limiter: options.limiter,
            node_id: options.node_id,
            syn_cookies: options.syn_cookies,
            workers: options.workers,
            publish_filter: options.publish_filter,
            tcp_server_factory,
            observer: options.observer,
            async_rt: Arc::downgrade(&options.async_rt),
        }
    }

    pub fn new_test_instance() -> Self {
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

    pub fn not_a_peer(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        endpoint.ip().is_unspecified()
            || reserved_address(endpoint, allow_local_peers)
            || endpoint
                == &SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port.load(Ordering::SeqCst), 0, 0)
    }

    pub fn on_new_channel(&self, callback: Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>) {
        self.tcp_channels.lock().unwrap().new_channel_observer = Some(callback);
    }

    pub fn insert(
        &self,
        channel: &Arc<ChannelEnum>,
        server: Option<Arc<TcpServer>>,
    ) -> Result<(), ()> {
        let ChannelEnum::Tcp(tcp_channel) = channel.as_ref() else {
            panic!("not a tcp channel")
        };
        let endpoint = tcp_channel.remote_endpoint();
        if !self.not_a_peer(&endpoint, self.allow_local_peers)
            && !self.stopped.load(Ordering::SeqCst)
        {
            let mut lock = self.tcp_channels.lock().unwrap();
            if !lock.channels.exists(&endpoint) {
                let node_id = channel.get_node_id().unwrap_or_default();
                if !channel.is_temporary() {
                    lock.channels.remove_by_node_id(&node_id);
                }

                let wrapper = Arc::new(ChannelEntry::new(channel.clone(), server));
                lock.channels.insert(wrapper);
                lock.attempts.remove(&endpoint);
                let observer = lock.new_channel_observer.clone();
                drop(lock);
                if let Some(callback) = observer {
                    callback(channel.clone());
                }
                return Ok(());
            }
        }
        Err(())
    }

    pub fn find_channel(&self, endpoint: &SocketAddrV6) -> Option<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().find_channel(endpoint)
    }

    pub fn random_channels(
        &self,
        count: usize,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().random_channels(
            count,
            min_version,
            include_temporary_channels,
        )
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

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        self.tcp_channels
            .lock()
            .unwrap()
            .collect_container_info(name)
    }

    pub fn erase_temporary_channel(&self, endpoint: &SocketAddrV6) {
        self.tcp_channels
            .lock()
            .unwrap()
            .erase_temporary_channel(endpoint);
    }

    pub fn random_fill(&self, endpoints: &mut [SocketAddrV6]) {
        self.tcp_channels.lock().unwrap().random_fill(endpoints);
    }

    pub fn random_fanout(&self, scale: f32) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().random_fanout(scale)
    }

    pub fn random_list(
        &self,
        count: usize,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().random_channels(
            count,
            min_version,
            include_temporary_channels,
        )
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

    pub fn set_observer(&self, observer: Weak<dyn TcpServerObserver>) {
        self.tcp_channels
            .lock()
            .unwrap()
            .tcp_server_factory
            .lock()
            .unwrap()
            .observer = observer;
    }

    pub fn set_message_visitor_factory(
        &self,
        visitor_factory: Arc<BootstrapMessageVisitorFactory>,
    ) {
        self.tcp_channels
            .lock()
            .unwrap()
            .tcp_server_factory
            .lock()
            .unwrap()
            .message_visitor_factory = Some(visitor_factory);
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
                .inc(StatType::Tcp, DetailType::TcpMaxPerIp, Direction::Out);
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
            self.stats.inc(
                StatType::Handshake,
                DetailType::InvalidNodeId,
                Direction::In,
            );
            return false; // Fail
        }

        // Prevent mismatched genesis
        if let Some(v2) = &response.v2 {
            if v2.genesis != self.network.ledger.genesis.hash() {
                self.stats.inc(
                    StatType::Handshake,
                    DetailType::InvalidGenesis,
                    Direction::In,
                );
                return false; // Fail
            }
        }

        let Some(cookie) = self.syn_cookies.cookie(&remote_endpoint) else {
            self.stats.inc(
                StatType::Handshake,
                DetailType::MissingCookie,
                Direction::In,
            );
            return false; // Fail
        };

        if response.validate(&cookie).is_err() {
            self.stats.inc(
                StatType::Handshake,
                DetailType::InvalidSignature,
                Direction::In,
            );
            return false; // Fail
        }

        self.stats
            .inc(StatType::Handshake, DetailType::Ok, Direction::In);
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
            self.stats.inc(
                StatType::Tcp,
                DetailType::TcpMaxPerSubnetwork,
                Direction::Out,
            );
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
        // Don't keepalive to nodes that already sent us something
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

    pub fn count(&self) -> usize {
        self.tcp_channels.lock().unwrap().channels.len()
    }

    pub fn bootstrap_peer(&self) -> SocketAddrV6 {
        self.tcp_channels.lock().unwrap().bootstrap_peer()
    }

    pub fn list_channels(
        &self,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels
            .lock()
            .unwrap()
            .list(min_version, include_temporary_channels)
    }

    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::SeqCst);
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
}

pub trait TcpChannelsExtension {
    fn process_messages(&self);
    fn process_message(
        &self,
        message: &DeserializedMessage,
        endpoint: &SocketAddrV6,
        node_id: PublicKey,
        socket: &Arc<Socket>,
    );

    fn merge_peer(&self, peer: &SocketAddrV6);
    fn keepalive(&self);
    fn start_tcp_receive_node_id(&self, channel: &Arc<ChannelEnum>, endpoint: SocketAddrV6);
    fn start_tcp(&self, endpoint: SocketAddrV6);
}

impl TcpChannelsExtension for Arc<TcpChannels> {
    fn process_messages(&self) {
        while !self.stopped.load(Ordering::SeqCst) {
            let item = self.tcp_message_manager.get_message();
            if let Some(message) = &item.message {
                self.process_message(message, &item.endpoint, item.node_id, &item.socket.unwrap());
            }
        }
    }

    fn process_message(
        &self,
        message: &DeserializedMessage,
        endpoint: &SocketAddrV6,
        node_id: PublicKey,
        socket: &Arc<Socket>,
    ) {
        let Some(async_rt) = self.async_rt.upgrade() else {
            return;
        };
        let socket_type = socket.socket_type();
        if !self.stopped.load(Ordering::SeqCst)
            && message.protocol.version_using >= self.network.network.protocol_version_min
        {
            if let Some(channel) = self.find_channel(endpoint) {
                (self.sink)(message.clone(), Arc::clone(&channel));
                channel.set_last_packet_received(SystemTime::now());
            } else {
                if let Some(channel) = self.find_node_id(&node_id) {
                    (self.sink)(message.clone(), Arc::clone(&channel));
                    channel.set_last_packet_received(SystemTime::now());
                } else if !self.excluded_peers.lock().unwrap().is_excluded(endpoint) {
                    if !node_id.is_zero() {
                        // Add temporary channel
                        let channel_id = self.get_next_channel_id();
                        let temporary_channel = Arc::new(ChannelTcp::new(
                            Arc::clone(socket),
                            SystemTime::now(),
                            Arc::clone(&self.stats),
                            self,
                            self.limiter.clone(),
                            &async_rt,
                            channel_id,
                            self.network.network.protocol_info(),
                        ));
                        temporary_channel.update_remote_endpoint();
                        debug_assert!(*endpoint == temporary_channel.remote_endpoint());
                        temporary_channel.set_node_id(node_id);
                        temporary_channel.set_network_version(message.protocol.version_using);
                        temporary_channel.set_temporary(true);
                        let temporary_channel = Arc::new(ChannelEnum::Tcp(temporary_channel));
                        debug_assert!(
                            socket_type == SocketType::Realtime
                                || socket_type == SocketType::RealtimeResponseServer,
                        );
                        // Don't insert temporary channels for response_server
                        if socket_type == SocketType::Realtime {
                            let _ = self.insert(&temporary_channel, None);
                        }
                        (self.sink)(message.clone(), temporary_channel);
                    } else {
                        // Initial node_id_handshake request without node ID
                        debug_assert!(
                            message.message.message_type() == MessageType::NodeIdHandshake
                        );
                        self.stats.inc(
                            StatType::Message,
                            DetailType::NodeIdHandshake,
                            Direction::In,
                        );
                    }
                }
            }
        }
    }

    fn merge_peer(&self, peer: &SocketAddrV6) {
        if !self.reachout_checked(peer, self.node_config.allow_local_peers) {
            self.start_tcp(*peer);
        }
    }

    fn keepalive(&self) {
        let mut peers = [SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0); 8];
        self.random_fill(&mut peers);
        let message = Message::Keepalive(Keepalive { peers });

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

    fn start_tcp_receive_node_id(&self, channel: &Arc<ChannelEnum>, endpoint: SocketAddrV6) {
        let this_w = Arc::downgrade(self);
        let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
            return;
        };
        let Some(socket_l) = tcp.socket() else { return };
        let channel_w = Arc::downgrade(channel);

        let channel_w_clone = channel_w.clone();
        let cleanup_node_id_handshake_socket = move || {
            if let Some(channel_l) = channel_w_clone.upgrade() {
                if let ChannelEnum::Tcp(tcp) = channel_l.as_ref() {
                    if let Some(socket_l) = tcp.socket() {
                        socket_l.close();
                    }
                }
            }
        };

        let channel_clone = channel.clone();
        let callback: Box<dyn FnOnce(ErrorCode, Option<DeserializedMessage>) + Send + Sync> =
            Box::new(move |ec, message| {
                let Some(this_l) = this_w.upgrade() else {
                    return;
                };
                let Some(message) = message else {
                    return;
                };
                let channel = channel_clone;
                let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
                    return;
                };

                if ec.is_err() {
                    debug!("Error reading node_id_handshake from: {}", endpoint);
                    cleanup_node_id_handshake_socket();
                    return;
                }
                this_l.stats.inc(
                    StatType::Message,
                    DetailType::NodeIdHandshake,
                    Direction::In,
                );

                // the header type should in principle be checked after checking the network bytes and the version numbers, I will not change it here since the benefits do not outweight the difficulties

                let Message::NodeIdHandshake(handshake) = &message.message else {
                    debug!(
                        "Error reading node_id_handshake message header from: {}",
                        endpoint
                    );
                    cleanup_node_id_handshake_socket();
                    return;
                };

                if message.protocol.network != this_l.network.network.current_network
                    || message.protocol.version_using < this_l.network.network.protocol_version_min
                {
                    // error handling, either the networks bytes or the version is wrong
                    if message.protocol.network == this_l.network.network.current_network {
                        this_l.stats.inc(
                            StatType::Message,
                            DetailType::InvalidNetwork,
                            Direction::In,
                        );
                    } else {
                        this_l.stats.inc(
                            StatType::Message,
                            DetailType::OutdatedVersion,
                            Direction::In,
                        );
                    }

                    cleanup_node_id_handshake_socket();
                    // Cleanup attempt
                    {
                        let mut guard = this_l.tcp_channels.lock().unwrap();
                        guard.attempts.remove(&endpoint);
                    }
                    return;
                }

                let invalid_handshake = || {
                    debug!("Error reading node_id_handshake from: {}", endpoint);
                    cleanup_node_id_handshake_socket();
                };

                let Some(response) = handshake.response.as_ref() else {
                    invalid_handshake();
                    return;
                };

                let Some(query) = handshake.query.as_ref() else {
                    invalid_handshake();
                    return;
                };

                tcp.set_network_version(message.protocol.version_using);

                let node_id = response.node_id;

                if !this_l.verify_handshake_response(response, endpoint) {
                    cleanup_node_id_handshake_socket();
                    return;
                }

                /* If node ID is known, don't establish new connection
                Exception: temporary channels from tcp_server */
                if let Some(existing_channel) = this_l.find_node_id(&node_id) {
                    if !existing_channel.is_temporary() {
                        cleanup_node_id_handshake_socket();
                        return;
                    }
                }
                tcp.set_node_id(node_id);
                tcp.set_last_packet_received(SystemTime::now());

                let response = this_l.prepare_handshake_response(query, handshake.is_v2);
                let handshake_response = Message::NodeIdHandshake(NodeIdHandshake {
                    query: None,
                    is_v2: response.v2.is_some(),
                    response: Some(response),
                });

                debug!(
                    "Node ID handshake response sent to {} (query: {:?})",
                    endpoint, query.cookie
                );

                let channel_clone = channel.clone();
                tcp.send(
                    &handshake_response,
                    Some(Box::new(move |ec, _| {
                        let Some(this_l) = this_w.upgrade() else {
                            return;
                        };
                        let channel = channel_clone;
                        let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
                            return;
                        };
                        if ec.is_err() {
                            debug!(
                                "Error sending node_id_handshake to: {} ({:?})",
                                endpoint, ec
                            );
                            cleanup_node_id_handshake_socket();
                            return;
                        }
                        // Insert new node ID connection
                        let response_server = this_l
                            .tcp_server_factory
                            .lock()
                            .unwrap()
                            .create_tcp_server(&tcp, Arc::clone(&tcp.socket));
                        let _ = this_l.insert(&channel, Some(response_server));
                    })),
                    BufferDropPolicy::Limiter,
                    TrafficType::Generic,
                );
            });

        if let Some(rt) = self.async_rt.upgrade() {
            let deserializer = Arc::new(MessageDeserializer::new(
                self.network.network.protocol_info(),
                self.network.work.clone(),
                self.publish_filter.clone(),
                socket_l,
            ));

            let stats = Arc::clone(&self.stats);
            rt.tokio.spawn(async move {
                let result = deserializer.read().await;
                spawn_blocking(Box::new(move || {
                    match &result {
                        Ok(payload) => stats.inc(
                            StatType::Message,
                            payload.message.message_type().into(),
                            Direction::In,
                        ),
                        Err(ParseMessageError::InsufficientWork) => stats.inc(
                            StatType::Filter,
                            DetailType::DuplicatePublishMessage,
                            Direction::In,
                        ),
                        Err(e) => stats.inc(StatType::Error, (*e).into(), Direction::In),
                    }

                    match result {
                        Ok(msg) => callback(ErrorCode::new(), Some(msg)),
                        Err(ParseMessageError::DuplicatePublishMessage) => {
                            callback(ErrorCode::new(), None)
                        }
                        Err(ParseMessageError::InsufficientWork) => {
                            callback(ErrorCode::new(), None)
                        }
                        Err(_) => callback(ErrorCode::fault(), None),
                    }
                }));
            });
        }
    }

    fn start_tcp(&self, endpoint: SocketAddrV6) {
        let Some(async_rt) = self.async_rt.upgrade() else {
            return;
        };
        let socket_stats = Arc::new(SocketStats::new(self.stats.clone()));

        let socket = SocketBuilder::endpoint_type(
            EndpointType::Client,
            self.workers.clone(),
            Weak::clone(&self.async_rt),
        )
        .default_timeout(Duration::from_secs(
            self.node_config.tcp_io_timeout_s as u64,
        ))
        .silent_connection_tolerance_time(Duration::from_secs(
            self.network.network.silent_connection_tolerance_time_s as u64,
        ))
        .idle_timeout(Duration::from_secs(
            self.network.network.idle_timeout_s as u64,
        ))
        .observer(Arc::new(CompositeSocketObserver::new(vec![
            socket_stats,
            self.observer.clone(),
        ])))
        .build();

        let channel_id = self.get_next_channel_id();
        let channel = Arc::new(ChannelEnum::Tcp(Arc::new(ChannelTcp::new(
            Arc::clone(&socket),
            SystemTime::now(),
            Arc::clone(&self.stats),
            self,
            self.limiter.clone(),
            &async_rt,
            channel_id,
            self.network.network.protocol_info(),
        ))));
        let this_w = Arc::downgrade(self);
        let socket_clone = Arc::clone(&socket);
        socket.async_connect(
            endpoint,
            Box::new(move |ec| {
                let _socket = socket_clone; //keep socket alive!
                let Some(this_l) = this_w.upgrade() else {
                    return;
                };

                if ec.is_err() {
                    debug!("Error connecting to: {} ({:?})", endpoint, ec);
                    return;
                }

                // TCP node ID handshake
                let query = this_l.prepare_handshake_query(endpoint);
                let message = Message::NodeIdHandshake(NodeIdHandshake {
                    query: query.clone(),
                    response: None,
                    is_v2: query.is_some(),
                });

                let query_string = query
                    .map(|q| format!("{:?}", q.cookie))
                    .unwrap_or_else(|| "not_set".to_string());
                debug!("Handshake sent to: {} (query {})", endpoint, query_string);

                let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
                    panic!("not a tcp channel")
                };
                tcp.update_remote_endpoint();
                let this_w = Arc::downgrade(&this_l);
                let channel_clone = Arc::clone(&channel);
                tcp.send(
                    &message,
                    Some(Box::new(move |ec, _size| {
                        let channel = channel_clone;
                        let ChannelEnum::Tcp(tcp) = channel.as_ref() else {
                            return;
                        };
                        if let Some(this_l) = this_w.upgrade() {
                            if ec.is_ok() {
                                this_l.start_tcp_receive_node_id(&channel, endpoint);
                            } else {
                                if let Some(socket) = tcp.socket() {
                                    socket.close();
                                }
                                debug!(
                                    "Error sending node_id_handshake to: {} ({:?})",
                                    endpoint, ec
                                );
                            }
                        }
                    })),
                    BufferDropPolicy::Limiter,
                    TrafficType::Generic,
                );
            }),
        );
    }
}

pub struct TcpChannelsImpl {
    pub attempts: TcpEndpointAttemptContainer,
    pub channels: ChannelContainer,
    network_constants: NetworkConstants,
    new_channel_observer: Option<Arc<dyn Fn(Arc<ChannelEnum>) + Send + Sync>>,
    pub tcp_server_factory: Arc<Mutex<TcpServerFactory>>,
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
            channel.socket().close();
            // Remove response server
            if let Some(server) = &channel.response_server {
                server.stop();
            }
        }
        self.channels.clear();
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        debug!("Performing periodic channel cleanup");
        self.channels.close_idle_channels(cutoff);

        // Check if any tcp channels belonging to old protocol versions which may still be alive due to async operations
        self.channels
            .close_old_protocol_versions(self.network_constants.protocol_version_min);

        // Remove channels with dead underlying sockets
        self.channels.remove_dead();

        // Remove keepalive attempt tracking for attempts older than cutoff
        self.attempts.purge(cutoff);
    }

    pub fn random_channels(
        &self,
        count: usize,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        let mut channels = self.list(min_version, include_temporary_channels);
        let mut rng = thread_rng();
        channels.shuffle(&mut rng);
        if count > 0 {
            channels.truncate(count)
        }
        channels
    }

    pub fn list(&self, min_version: u8, include_temporary_channels: bool) -> Vec<Arc<ChannelEnum>> {
        self.channels
            .iter()
            .filter(|c| {
                c.tcp_channel().network_version() >= min_version
                    && (include_temporary_channels || !c.channel.is_temporary())
                    && c.channel.is_alive()
            })
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

    pub fn erase_temporary_channel(&mut self, endpoint: &SocketAddrV6) {
        if let Some(channel) = self.channels.remove_by_endpoint(endpoint) {
            channel.set_temporary(false);
        }
    }

    pub fn random_fanout(&self, scale: f32) -> Vec<Arc<ChannelEnum>> {
        self.random_channels(self.fanout(scale), 0, true)
    }

    pub fn random_fill(&self, endpoints: &mut [SocketAddrV6]) {
        // Don't include channels with ephemeral remote ports
        let peers = self.random_channels(endpoints.len(), 0, false);
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
                    debug!("Closing channel with old protocol version: {}", ep);
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
    pub last_attempt: SystemTime,
}

impl AttemptEntry {
    pub fn new(endpoint: SocketAddrV6) -> Self {
        Self {
            endpoint,
            address: ipv4_address_or_ipv6_subnet(endpoint.ip()),
            subnetwork: map_address_to_subnetwork(endpoint.ip()),
            last_attempt: SystemTime::now(),
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
            .entry(attempt.last_attempt)
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

            let by_time = self.by_time.get_mut(&attempt.last_attempt).unwrap();
            if by_time.len() > 1 {
                by_time.retain(|x| x != endpoint);
            } else {
                self.by_time.remove(&attempt.last_attempt);
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
