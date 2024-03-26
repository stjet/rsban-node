use super::{
    CompositeSocketObserver, ConnectionsPerAddress, EndpointType, NullSocketObserver, Socket,
    SocketBuilder, SocketExtensions, SocketObserver, SynCookies, TcpChannels, TcpServer,
    TcpServerExt, TcpServerObserver, TcpSocketFacadeFactory, TokioSocketFacade,
    TokioSocketFacadeFactory,
};
use crate::{
    block_processing::BlockProcessor,
    bootstrap::{BootstrapInitiator, BootstrapMessageVisitorFactory},
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    utils::{
        first_ipv6_subnet_address, is_ipv4_mapped, AsyncRuntime, ErrorCode, ThreadPool,
        ThreadPoolImpl,
    },
    NetworkParams, DEV_NETWORK_PARAMS,
};
use rsnano_core::KeyPair;
use rsnano_ledger::Ledger;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    ops::Deref,
    sync::{
        atomic::{AtomicU16, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tracing::{debug, error, warn};

pub struct TcpListener {
    port: AtomicU16,
    max_inbound_connections: usize,
    config: NodeConfig,
    tcp_channels: Weak<TcpChannels>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    runtime: Arc<AsyncRuntime>,
    socket_observer: Arc<dyn SocketObserver>,
    workers: Arc<dyn ThreadPool>,
    tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    network_params: NetworkParams,
    node_flags: NodeFlags,
    socket_facade: Arc<TokioSocketFacade>,
    data: Mutex<TcpListenerData>,
    ledger: Arc<Ledger>,
    block_processor: Arc<BlockProcessor>,
    bootstrap_initiator: Arc<BootstrapInitiator>,
    node_id: Arc<KeyPair>,
    bootstrap_count: AtomicUsize,
    realtime_count: AtomicUsize,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        debug_assert!(self.data.lock().unwrap().stopped);
    }
}

struct TcpListenerData {
    connections: HashMap<usize, Weak<TcpServer>>,
    stopped: bool,
    listening_socket: Option<Arc<ServerSocket>>, // TODO remove arc
}

struct ServerSocket {
    socket: Arc<Socket>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
}

impl TcpListenerData {
    fn evict_dead_connections(&self) {
        let Some(socket) = &self.listening_socket else {
            return;
        };

        socket
            .connections_per_address
            .lock()
            .unwrap()
            .evict_dead_connections();
    }
}

impl TcpListener {
    pub fn new(
        port: u16,
        max_inbound_connections: usize,
        config: NodeConfig,
        tcp_channels: Arc<TcpChannels>,
        syn_cookies: Arc<SynCookies>,
        network_params: NetworkParams,
        node_flags: NodeFlags,
        runtime: Arc<AsyncRuntime>,
        socket_observer: Arc<dyn SocketObserver>,
        stats: Arc<Stats>,
        workers: Arc<dyn ThreadPool>,
        block_processor: Arc<BlockProcessor>,
        bootstrap_initiator: Arc<BootstrapInitiator>,
        ledger: Arc<Ledger>,
        node_id: Arc<KeyPair>,
    ) -> Self {
        let tcp_socket_facade_factory =
            Arc::new(TokioSocketFacadeFactory::new(Arc::clone(&runtime)));
        Self {
            port: AtomicU16::new(port),
            max_inbound_connections,
            config,
            tcp_channels: Arc::downgrade(&tcp_channels),
            syn_cookies,
            data: Mutex::new(TcpListenerData {
                connections: HashMap::new(),
                stopped: false,
                listening_socket: None,
            }),
            network_params,
            node_flags,
            runtime: Arc::clone(&runtime),
            socket_facade: Arc::new(TokioSocketFacade::create(runtime)),
            socket_observer,
            tcp_socket_facade_factory,
            stats,
            workers,
            block_processor,
            bootstrap_initiator,
            bootstrap_count: AtomicUsize::new(0),
            realtime_count: AtomicUsize::new(0),
            ledger,
            node_id,
        }
    }

    pub fn new_test_builder(ledger: Arc<Ledger>) -> TcpListenerBuilder {
        TcpListenerBuilder::new(ledger)
    }

    pub fn stop(&self) {
        let mut conns = HashMap::new();
        {
            let mut guard = self.data.lock().unwrap();
            guard.stopped = true;
            std::mem::swap(&mut conns, &mut guard.connections);

            if let Some(socket) = guard.listening_socket.take() {
                socket.socket.close_internal();
                self.socket_facade.close_acceptor();
                socket
                    .connections_per_address
                    .lock()
                    .unwrap()
                    .close_connections();
            }
        }
    }

    pub fn get_realtime_count(&self) -> usize {
        self.realtime_count.load(Ordering::SeqCst)
    }

    pub fn connection_count(&self) -> usize {
        let mut data = self.data.lock().unwrap();
        cleanup(&mut data);
        data.connections.len()
    }

    pub fn endpoint(&self) -> SocketAddrV6 {
        let guard = self.data.lock().unwrap();
        if !guard.stopped && guard.listening_socket.is_some() {
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port.load(Ordering::SeqCst), 0, 0)
        } else {
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)
        }
    }

    fn remove_connection(&self, connection_id: usize) {
        let mut data = self.data.lock().unwrap();
        data.connections.remove(&connection_id);
    }

    fn limit_reached_for_incoming_subnetwork_connections(
        &self,
        new_connection: &Arc<Socket>,
    ) -> bool {
        let endpoint = new_connection
            .get_remote()
            .expect("new connection has no remote endpoint set");
        if self.node_flags.disable_max_peers_per_subnetwork || is_ipv4_mapped(&endpoint.ip()) {
            // If the limit is disabled, then it is unreachable.
            // If the address is IPv4 we don't check for a network limit, since its address space isn't big as IPv6 /64.
            return false;
        }

        let guard = self.data.lock().unwrap();
        let Some(socket) = &guard.listening_socket else {
            return false;
        };

        let counted_connections = socket
            .connections_per_address
            .lock()
            .unwrap()
            .count_subnetwork_connections(
                endpoint.ip(),
                self.network_params
                    .network
                    .ipv6_subnetwork_prefix_for_limiting,
            );

        counted_connections >= self.network_params.network.max_peers_per_subnetwork
    }

    fn limit_reached_for_incoming_ip_connections(&self, new_connection: &Arc<Socket>) -> bool {
        if self.node_flags.disable_max_peers_per_ip {
            return false;
        }

        let guard = self.data.lock().unwrap();
        let Some(socket) = &guard.listening_socket else {
            return false;
        };

        let ip = new_connection.get_remote().unwrap().ip().clone();
        let counted_connections = socket
            .connections_per_address
            .lock()
            .unwrap()
            .count_connections_for_ip(&ip);

        counted_connections >= self.network_params.network.max_peers_per_ip
    }
}

fn cleanup(data: &mut std::sync::MutexGuard<'_, TcpListenerData>) {
    data.connections.retain(|_, conn| conn.strong_count() > 0)
}

pub trait TcpListenerExt {
    /// If we are unable to accept a socket, for any reason, we wait just a little (1ms) before rescheduling the next connection accept.
    /// The intention is to throttle back the connection requests and break up any busy loops that could possibly form and
    /// give the rest of the system a chance to recover.
    fn on_connection_requeue_delayed(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    );
    fn start(&self) -> anyhow::Result<()>;
    fn start_with(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) -> anyhow::Result<()>;
    fn on_connection(&self, callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>);
    fn accept_action(&self, ec: ErrorCode, socket: Arc<Socket>);
    fn as_observer(self) -> Arc<dyn TcpServerObserver>;
}

impl TcpListenerExt for Arc<TcpListener> {
    fn on_connection_requeue_delayed(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) {
        let this_w = Arc::downgrade(self);
        self.workers.add_delayed_task(
            Duration::from_millis(1),
            Box::new(move || {
                if let Some(this_l) = this_w.upgrade() {
                    this_l.on_connection(callback);
                }
            }),
        );
    }
    fn start(&self) -> anyhow::Result<()> {
        let self_w = Arc::downgrade(&self);
        self.start_with(Box::new(move |socket, ec| {
            if let Some(listener) = self_w.upgrade() {
                listener.accept_action(ec, socket);
                true
            } else {
                false
            }
        }))
    }

    fn start_with(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock().unwrap();

        let socket_stats = Arc::new(SocketStats::new(Arc::clone(&self.stats)));

        let socket = SocketBuilder::endpoint_type(
            EndpointType::Server,
            Arc::clone(&self.workers),
            Arc::downgrade(&self.runtime),
        )
        .default_timeout(Duration::MAX)
        .silent_connection_tolerance_time(Duration::from_secs(
            self.network_params
                .network
                .silent_connection_tolerance_time_s as u64,
        ))
        .idle_timeout(Duration::from_secs(
            self.network_params.network.idle_timeout_s as u64,
        ))
        .observer(Arc::new(CompositeSocketObserver::new(vec![
            socket_stats,
            Arc::clone(&self.socket_observer),
        ])))
        .build();

        let listening_socket = Arc::new(ServerSocket {
            socket,
            connections_per_address: Mutex::new(Default::default()),
        });

        let ec = self.socket_facade.open(&SocketAddr::new(
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            self.port.load(Ordering::SeqCst),
        ));
        let listening_port = self.socket_facade.listening_port();
        if ec.is_err() {
            error!(
                "Error while binding for incoming TCP/bootstrap on port {}: {:?}",
                listening_port, ec
            );
            bail!("Network: Error while binding for incoming TCP/bootstrap");
        }

        // the user can either specify a port value in the config or it can leave the choice up to the OS:
        // (1): port specified
        // (2): port not specified

        // (1) -- nothing to do
        //
        if self.port.load(Ordering::SeqCst) == listening_port {
        }
        // (2) -- OS port choice happened at TCP socket bind time, so propagate this port value back;
        // the propagation is done here for the `tcp_listener` itself, whereas for `network`, the node does it
        // after calling `tcp_listener.start ()`
        //
        else {
            self.port.store(listening_port, Ordering::SeqCst);
        }
        data.listening_socket = Some(listening_socket);
        drop(data);
        self.on_connection(callback);
        Ok(())
    }
    fn on_connection(&self, callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>) {
        let listening_socket = {
            let guard = self.data.lock().unwrap();
            let Some(s) = &guard.listening_socket else {
                return;
            };
            Arc::clone(s)
        };
        let this_w = Arc::downgrade(self);
        self.socket_facade.post(Box::new(move || {
            let Some(this_l) = this_w.upgrade() else {return;};
            if !this_l.socket_facade.is_acceptor_open() {
                error!("Socket acceptor is not open");
                return;
            }

            let socket_stats = Arc::new(SocketStats::new(
                Arc::clone(&this_l.stats),
            ));

            // Prepare new connection
            let new_connection = SocketBuilder::endpoint_type(
                EndpointType::Server,
                Arc::clone(&this_l.workers),
                Arc::downgrade(&this_l.runtime),
            )
            .default_timeout(Duration::from_secs(
                this_l.config.tcp_io_timeout_s as u64,
            ))
            .idle_timeout(Duration::from_secs(
                this_l
                    .network_params
                    .network
                    .silent_connection_tolerance_time_s as u64,
            ))
            .silent_connection_tolerance_time(Duration::from_secs(
                this_l
                    .network_params
                    .network
                    .silent_connection_tolerance_time_s as u64,
            ))
            .observer(Arc::new(CompositeSocketObserver::new(vec![
                socket_stats,
                Arc::clone(&this_l.socket_observer),
            ])))
            .build();

            let listening_socket_clone = Arc::clone(&listening_socket);
            let connection_clone = Arc::clone(&new_connection);
            let this_clone_w = Arc::downgrade(&this_l);
            this_l.socket_facade.async_accept(
                &new_connection,
                Box::new(move |remote_endpoint, ec| {
                    let Some(this_clone) = this_clone_w.upgrade() else { return; };
                    let SocketAddr::V6(remote_endpoint) = remote_endpoint else {panic!("not a v6 address")};
                    let socket_l = listening_socket_clone;
                    connection_clone.set_remote(remote_endpoint);

                    let data = this_clone.data.lock().unwrap();
                    data.evict_dead_connections();
                    drop(data);

                    if socket_l.connections_per_address.lock().unwrap().count_connections() >= this_clone.max_inbound_connections {
                        this_clone.stats.inc (StatType::Tcp, DetailType::TcpAcceptFailure, Direction::In);
                        debug!("Max_inbound_connections reached, unable to open new connection");

                        this_clone.on_connection_requeue_delayed (callback);
                        return;
                    }

                    if this_clone.limit_reached_for_incoming_ip_connections (&connection_clone) {
                        let remote_ip_address = connection_clone.get_remote().unwrap().ip().clone();
                        this_clone.stats.inc (StatType::Tcp, DetailType::TcpMaxPerIp, Direction::In);
                        debug!("Max connections per IP (max_peers_per_ip) was reached for {}, unable to open new connection", remote_ip_address);
                        this_clone.on_connection_requeue_delayed (callback);
                        return;
                    }

                    if this_clone.limit_reached_for_incoming_subnetwork_connections (&connection_clone) {
                        let remote_ip_address = connection_clone.get_remote().unwrap().ip().clone();
                        let remote_subnet = first_ipv6_subnet_address(&remote_ip_address, this_clone.network_params.network.max_peers_per_subnetwork as u8);
                        this_clone.stats.inc(StatType::Tcp, DetailType::TcpMaxPerSubnetwork, Direction::In);
                        debug!("Max connections per subnetwork (max_peers_per_subnetwork) was reached for subnetwork {} (remote IP: {}), unable to open new connection",
                            remote_subnet, remote_ip_address);
                        this_clone.on_connection_requeue_delayed (callback);
                        return;
                    }
                   			if ec.is_ok() {
                    				// Make sure the new connection doesn't idle. Note that in most cases, the callback is going to start
                    				// an IO operation immediately, which will start a timer.
                    				connection_clone.start ();
                    				connection_clone.set_timeout (Duration::from_secs(this_clone.network_params.network.idle_timeout_s as u64));
                    				this_clone.stats.inc (StatType::Tcp, DetailType::TcpAcceptSuccess, Direction::In);
                                    socket_l.connections_per_address.lock().unwrap().insert(&connection_clone);
                                    this_clone.socket_observer.socket_accepted(Arc::clone(&connection_clone));
                    				if callback (connection_clone, ec)
                    				{
                    					this_clone.on_connection (callback);
                    					return;
                    				}
                    				warn!("Stopping to accept connections");
                    				return;
                   			}

                    			// accept error
                    			this_clone.stats.inc (StatType::Tcp, DetailType::TcpAcceptFailure, Direction::In);
                    			error!("Unable to accept connection: ({:?})", ec);

                    			if is_temporary_error (ec)
                    			{
                    				// if it is a temporary error, just retry it
                    				this_clone.on_connection_requeue_delayed (callback);
                    				return;
                    			}

                    			// if it is not a temporary error, check how the listener wants to handle this error
                    			if callback(connection_clone, ec)
                    			{
                    				this_clone.on_connection_requeue_delayed (callback);
                    				return;
                    			}

                    			// No requeue if we reach here, no incoming socket connections will be handled
                    			warn!("Stopping to accept connections");
                }),
            );
        }));
    }

    fn accept_action(&self, _ec: ErrorCode, socket: Arc<Socket>) {
        let Some(remote) = socket.get_remote() else {
            return;
        };
        let Some(tcp_channels) = self.tcp_channels.upgrade() else {
            return;
        };
        if !tcp_channels
            .excluded_peers
            .lock()
            .unwrap()
            .is_excluded(&remote)
        {
            let message_visitor_factory = Arc::new(BootstrapMessageVisitorFactory::new(
                Arc::clone(&self.runtime),
                Arc::clone(&self.syn_cookies),
                Arc::clone(&self.stats),
                self.network_params.network.clone(),
                Arc::clone(&self.node_id),
                Arc::clone(&self.ledger),
                Arc::clone(&self.workers),
                Arc::clone(&self.block_processor),
                Arc::clone(&self.bootstrap_initiator),
                self.node_flags.clone(),
            ));
            let observer = Arc::downgrade(&self);
            let server = Arc::new(TcpServer::new(
                Arc::clone(&self.runtime),
                socket,
                Arc::new(self.config.clone()),
                observer,
                Arc::clone(&tcp_channels.publish_filter),
                Arc::new(self.network_params.clone()),
                Arc::clone(&self.stats),
                Arc::clone(&tcp_channels.tcp_message_manager),
                message_visitor_factory,
                true,
                Arc::clone(&self.syn_cookies),
                self.node_id.deref().clone(),
            ));

            let mut data = self.data.lock().unwrap();
            data.connections
                .insert(server.unique_id(), Arc::downgrade(&server));
            server.start();
        } else {
            self.stats
                .inc(StatType::Tcp, DetailType::TcpExcluded, Direction::In);
            debug!("Rejected connection from excluded peer {}", remote);
        }
    }

    fn as_observer(self) -> Arc<dyn TcpServerObserver> {
        self
    }
}

pub struct TcpListenerBuilder {
    port: u16,
    tcp_channels: Option<Arc<TcpChannels>>,
    syn_cookies: Option<Arc<SynCookies>>,
    async_rt: Option<Arc<AsyncRuntime>>,
    ledger: Arc<Ledger>,
    node_id: Option<KeyPair>,
    block_processor: Option<Arc<BlockProcessor>>,
}

impl TcpListenerBuilder {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            ledger,
            port: 8088,
            tcp_channels: None,
            syn_cookies: None,
            async_rt: None,
            node_id: None,
            block_processor: None,
        }
    }

    pub fn async_rt(mut self, async_rt: Arc<AsyncRuntime>) -> Self {
        self.async_rt = Some(async_rt);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn tcp_channels(mut self, channels: Arc<TcpChannels>) -> Self {
        self.tcp_channels = Some(channels);
        self
    }

    pub fn syn_cookies(mut self, syn_cookies: Arc<SynCookies>) -> Self {
        self.syn_cookies = Some(syn_cookies);
        self
    }

    pub fn node_id(mut self, node_id: KeyPair) -> Self {
        self.node_id = Some(node_id);
        self
    }

    pub fn block_processor(mut self, processor: Arc<BlockProcessor>) -> Self {
        self.block_processor = Some(processor);
        self
    }

    pub fn build(self) -> TcpListener {
        let block_processor = self.block_processor.unwrap_or_else(|| {
            Arc::new(BlockProcessor::new_test_instance(Arc::clone(&self.ledger)))
        });
        let bootstrap_initiator = Arc::new(BootstrapInitiator::new(std::ptr::null_mut()));

        TcpListener::new(
            self.port,
            32,
            NodeConfig::new_null(),
            self.tcp_channels
                .unwrap_or_else(|| Arc::new(TcpChannels::new_test_instance())),
            self.syn_cookies
                .unwrap_or_else(|| Arc::new(SynCookies::default())),
            DEV_NETWORK_PARAMS.clone(),
            NodeFlags::default(),
            self.async_rt
                .unwrap_or_else(|| Arc::new(AsyncRuntime::default())),
            Arc::new(NullSocketObserver::new()),
            Arc::new(Stats::default()),
            Arc::new(ThreadPoolImpl::new_test_instance()),
            block_processor,
            bootstrap_initiator,
            self.ledger,
            Arc::new(self.node_id.unwrap_or_else(|| KeyPair::new())),
        )
    }
}

impl TcpServerObserver for TcpListener {
    fn bootstrap_server_timeout(&self, connection_id: usize) {
        debug!("Closing TCP server due to timeout");
    }

    fn boostrap_server_exited(
        &self,
        socket_type: super::SocketType,
        _connection_id: usize,
        endpoint: SocketAddrV6,
    ) {
        debug!("Exiting TCP server ({})", endpoint);
        if socket_type == super::SocketType::Bootstrap {
            self.bootstrap_count.fetch_sub(1, Ordering::SeqCst);
        } else if socket_type == super::SocketType::Realtime {
            self.realtime_count.fetch_sub(1, Ordering::SeqCst);
            // Clear temporary channel
            if let Some(tcp_channels) = self.tcp_channels.upgrade() {
                tcp_channels.erase_temporary_channel(&endpoint);
            }
        }
    }

    fn get_bootstrap_count(&self) -> usize {
        self.bootstrap_count.load(Ordering::SeqCst)
    }

    fn inc_bootstrap_count(&self) {
        self.bootstrap_count.fetch_add(1, Ordering::SeqCst);
    }

    fn dec_bootstrap_count(&self) {
        self.bootstrap_count.fetch_sub(1, Ordering::SeqCst);
    }

    fn inc_realtime_count(&self) {
        self.realtime_count.fetch_add(1, Ordering::SeqCst);
    }

    fn dec_realtime_count(&self) {
        self.realtime_count.fetch_sub(1, Ordering::SeqCst);
    }
}

fn is_temporary_error(ec: ErrorCode) -> bool {
    return ec.val == 11 // would block
                        || ec.val ==  4; // interrupted system call
}
