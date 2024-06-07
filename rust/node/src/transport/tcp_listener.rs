use super::{
    CompositeSocketObserver, ConnectionDirection, ConnectionsPerAddress, Socket, SocketBuilder,
    SocketExtensions, SocketObserver, SynCookies, TcpChannels, TcpConfig, TcpServer, TcpServerExt,
    TcpServerObserver, TcpSocketFacadeFactory, TokioSocketFacade, TokioSocketFacadeFactory,
};
use crate::{
    block_processing::BlockProcessor,
    bootstrap::{BootstrapInitiator, BootstrapMessageVisitorFactory},
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    utils::{
        into_ipv6_socket_address, is_ipv4_or_v4_mapped_address, map_address_to_subnetwork,
        AsyncRuntime, ErrorCode, ThreadPool,
    },
    NetworkParams,
};
use async_trait::async_trait;
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    KeyPair,
};
use rsnano_ledger::Ledger;
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    ops::Deref,
    sync::{
        atomic::{AtomicU16, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, Weak,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tracing::{debug, error, info, warn};

#[derive(PartialEq, Eq)]
pub enum AcceptResult {
    Invalid,
    Accepted,
    Rejected,
    Error,
}

pub struct AcceptReturn {
    result: AcceptResult,
    socket: Option<Arc<Socket>>,
    server: Option<Arc<TcpServer>>,
}

impl AcceptReturn {
    fn error() -> Self {
        Self::failed(AcceptResult::Error)
    }

    fn failed(result: AcceptResult) -> Self {
        Self {
            result,
            socket: None,
            server: None,
        }
    }
}

struct Connection {
    endpoint: SocketAddrV6,
    socket: Weak<Socket>,
    server: Weak<TcpServer>,
}

/// Server side portion of tcp sessions. Listens for new socket connections and spawns tcp_server objects when connected.
pub struct TcpListener {
    port: AtomicU16,
    config: TcpConfig,
    node_config: NodeConfig,
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
    cleanup_thread: Mutex<Option<JoinHandle<()>>>,
    condition: Condvar,
    close: Mutex<Option<oneshot::Sender<()>>>,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        debug_assert!(self.cleanup_thread.lock().unwrap().is_none());
        debug_assert!(self.data.lock().unwrap().stopped);
        debug_assert_eq!(self.connection_count(), 0);
        debug_assert_eq!(self.attempt_count(), 0);
    }
}

struct TcpListenerData {
    connections: Vec<Connection>,
    attempts: Vec<Attempt>,
    stopped: bool,
    local_addr: SocketAddr,
}

struct ServerSocket {
    socket: Arc<Socket>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
}

impl TcpListenerData {
    fn count_attempts(&self, ip: &Ipv6Addr) -> usize {
        self.attempts
            .iter()
            .filter(|a| a.endpoint.ip() == ip)
            .count()
    }
}

impl TcpListener {
    pub fn new(
        port: u16,
        config: TcpConfig,
        node_config: NodeConfig,
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
            config,
            node_config,
            tcp_channels: Arc::downgrade(&tcp_channels),
            syn_cookies,
            data: Mutex::new(TcpListenerData {
                connections: Vec::new(),
                attempts: Vec::new(),
                stopped: false,
                local_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
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
            cleanup_thread: Mutex::new(None),
            condition: Condvar::new(),
            close: Mutex::new(None),
        }
    }

    pub fn stop(&self) {
        // Close sockets
        let mut conns = Vec::new();
        {
            let mut guard = self.data.lock().unwrap();
            guard.stopped = true;
            std::mem::swap(&mut conns, &mut guard.connections);
        }

        if let Some(close) = self.close.lock().unwrap().take() {
            close.send(()).unwrap();
        }
        self.condition.notify_all();

        if let Some(handle) = self.cleanup_thread.lock().unwrap().take() {
            handle.join().unwrap();
        }

        // Close attempts
        {
            let mut attempts = Vec::new();
            {
                let mut guard = self.data.lock().unwrap();
                std::mem::swap(&mut guard.attempts, &mut attempts);
            }
            self.runtime.tokio.block_on(async {
                for attempt in attempts {
                    let _ = attempt.join_handle.await;
                }
            })
        }

        for conn in conns {
            if let Some(socket) = conn.socket.upgrade() {
                socket.close();
            }
            if let Some(server) = conn.server.upgrade() {
                server.stop();
            }
        }
    }

    pub fn realtime_count(&self) -> usize {
        self.realtime_count.load(Ordering::SeqCst)
    }

    pub fn connection_count(&self) -> usize {
        let mut data = self.data.lock().unwrap();
        self.cleanup(&mut data);
        data.connections.len()
    }

    pub fn attempt_count(&self) -> usize {
        self.data.lock().unwrap().attempts.len()
    }

    pub fn local_address(&self) -> SocketAddr {
        let guard = self.data.lock().unwrap();
        if !guard.stopped {
            guard.local_addr
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0)
        }
    }

    fn run_cleanup(&self) {
        let mut guard = self.data.lock().unwrap();
        while !guard.stopped {
            self.stats.inc(StatType::TcpListener, DetailType::Cleanup);

            self.cleanup(&mut guard);
            self.timeout(&mut guard);

            guard = self
                .condition
                .wait_timeout_while(guard, Duration::from_secs(1), |g| !g.stopped)
                .unwrap()
                .0;
        }
    }

    fn cleanup(&self, data: &mut TcpListenerData) {
        // Erase dead connections
        data.connections.retain(|conn| {
            let retain = conn.server.strong_count() > 0;
            if !retain {
                self.stats.inc(StatType::TcpListener, DetailType::EraseDead);
                debug!("Evicting dead connection");
            }
            retain
        });

        // Erase completed attempts
        data.attempts
            .retain(|attempt| !attempt.join_handle.is_finished())
    }

    fn timeout(&self, data: &mut TcpListenerData) {
        for attempt in &data.attempts {
            if !attempt.join_handle.is_finished()
                && attempt.start.elapsed() >= self.config.connect_timeout
            {
                attempt.join_handle.abort();
                self.stats
                    .inc(StatType::TcpListener, DetailType::AttemptTimeout);
                debug!(
                    "Connection attempt timed out: {} (started {}s ago)",
                    attempt.endpoint,
                    attempt.start.elapsed().as_secs()
                );
            }
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "connections".to_string(),
                count: self.connection_count(),
                sizeof_element: 1,
            })],
        )
    }

    fn check_limits(
        &self,
        ip: &Ipv6Addr,
        direction: ConnectionDirection,
        data: &mut TcpListenerData,
    ) -> AcceptResult {
        if data.stopped {
            return AcceptResult::Rejected;
        }

        let Some(channels) = self.tcp_channels.upgrade() else {
            return AcceptResult::Rejected;
        };

        self.cleanup(data);

        if let Some(channels) = self.tcp_channels.upgrade() {
            if channels.excluded_peers.lock().unwrap().is_excluded_ip(ip) {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::Excluded,
                    direction.into(),
                );

                debug!("Rejected connection from excluded peer: {}", ip);
                return AcceptResult::Rejected;
            }
        }

        if !self.node_flags.disable_max_peers_per_ip {
            let count = channels.count_per_ip(ip);
            if count >= self.network_params.network.max_peers_per_ip {
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
            let count = channels.count_per_subnetwork(ip);
            if count >= self.network_params.network.max_peers_per_subnetwork {
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
                // Should be checked earlier (wait_available_slots)
                debug_assert!(data.connections.len() <= self.config.max_inbound_connections);
                let count = channels.count_per_direction(ConnectionDirection::Inbound);
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
                let count = channels.count_per_direction(ConnectionDirection::Outbound);
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

    fn is_stopped(&self) -> bool {
        self.data.lock().unwrap().stopped
    }
}

#[async_trait]
pub trait TcpListenerExt {
    fn start(&self);
    async fn run(&self, listener: tokio::net::TcpListener, close: oneshot::Receiver<()>);
    fn connect_ip(&self, remote: Ipv6Addr) -> bool;
    fn connect(&self, remote: SocketAddrV6) -> bool;
    fn as_observer(self) -> Arc<dyn TcpServerObserver>;

    async fn connect_impl(&self, endpoint: SocketAddrV6) -> anyhow::Result<()>;

    async fn accept_one(
        &self,
        raw_stream: tokio::net::TcpStream,
        direction: ConnectionDirection,
    ) -> AcceptReturn;

    async fn wait_available_slots(&self);
}

#[async_trait]
impl TcpListenerExt for Arc<TcpListener> {
    /// Connects to the default peering port
    fn connect_ip(&self, remote: Ipv6Addr) -> bool {
        self.connect(SocketAddrV6::new(
            remote,
            self.network_params.network.default_node_port,
            0,
            0,
        ))
    }

    fn connect(&self, remote: SocketAddrV6) -> bool {
        let mut guard = self.data.lock().unwrap();

        let count = guard.attempts.len();
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

        let count = guard.count_attempts(remote.ip());
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

        if self.check_limits(remote.ip(), ConnectionDirection::Outbound, &mut guard)
            != AcceptResult::Accepted
        {
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

        let self_l = Arc::clone(self);
        let join_handle = self.runtime.tokio.spawn(async move {
            if let Err(e) = self_l.connect_impl(remote).await {
                self_l.stats.inc_dir(
                    StatType::TcpListener,
                    DetailType::ConnectError,
                    Direction::Out,
                );
                debug!("Error connecting to: {} ({:?})", remote, e);
            }
        });

        guard.attempts.push(Attempt {
            endpoint: remote,
            start: Instant::now(),
            join_handle,
        });
        true // Attempt started
    }

    fn start(&self) {
        let (close_tx, close_rx) = oneshot::channel();
        *self.close.lock().unwrap() = Some(close_tx);
        let self_l = Arc::clone(self);
        self.runtime.tokio.spawn(async move {
            let port = self_l.port.load(Ordering::SeqCst);
            let Ok(listener) = tokio::net::TcpListener::bind(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                port,
            ))
            .await
            else {
                error!("Error while binding for incoming connections on: {}", port);
                return;
            };

            let addr = listener
                .local_addr()
                .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0));
            info!("Listening for incoming connections on: {}", addr);

            if let Some(channels) = self_l.tcp_channels.upgrade() {
                channels.set_port(addr.port());
            }
            self_l.data.lock().unwrap().local_addr = addr;

            self_l.run(listener, close_rx).await
        });

        let self_w = Arc::downgrade(self);
        *self.cleanup_thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("TCP listener".to_owned())
                .spawn(move || {
                    if let Some(self_l) = self_w.upgrade() {
                        self_l.run_cleanup();
                    }
                })
                .unwrap(),
        );
    }

    async fn run(&self, listener: tokio::net::TcpListener, close: oneshot::Receiver<()>) {
        let run_loop = async {
            loop {
                self.wait_available_slots().await;

                let Ok((stream, _)) = listener.accept().await else {
                    self.stats.inc_dir(
                        StatType::TcpListener,
                        DetailType::AcceptFailure,
                        Direction::In,
                    );
                    continue;
                };

                let result = self.accept_one(stream, ConnectionDirection::Inbound).await;
                if result.result != AcceptResult::Accepted {
                    self.stats.inc_dir(
                        StatType::TcpListener,
                        DetailType::AcceptFailure,
                        Direction::In,
                    );
                    // Refusal reason should be logged earlier
                }

                // Sleep for a while to prevent busy loop
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };

        tokio::select! {
            _ = close =>{ },
            _ = run_loop => {}
        }
    }

    async fn wait_available_slots(&self) {
        let last_log = Instant::now();
        let log_interval = if self.network_params.network.is_dev_network() {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(15)
        };
        while self.connection_count() >= self.config.max_inbound_connections && !self.is_stopped() {
            if last_log.elapsed() >= log_interval {
                warn!(
                    "Waiting for available slots to accept new connections (current: {} / max: {})",
                    self.connection_count(),
                    self.config.max_inbound_connections
                );
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn as_observer(self) -> Arc<dyn TcpServerObserver> {
        self
    }

    async fn connect_impl(&self, endpoint: SocketAddrV6) -> anyhow::Result<()> {
        let raw_listener = tokio::net::TcpSocket::new_v6()?;
        let raw_stream = raw_listener.connect(endpoint.into()).await?;
        let result = self
            .accept_one(raw_stream, ConnectionDirection::Outbound)
            .await;
        if result.result == AcceptResult::Accepted {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::ConnectSuccess,
                Direction::Out,
            );
            debug!("Successfully connected to: {}", endpoint);
            result.server.unwrap().initiate_handshake();
        } else {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::ConnectFailure,
                Direction::Out,
            );
            // Refusal reason should be logged earlier
        }
        Ok(())
    }

    async fn accept_one(
        &self,
        mut raw_stream: tokio::net::TcpStream,
        direction: ConnectionDirection,
    ) -> AcceptReturn {
        let Ok(remote_endpoint) = raw_stream.peer_addr() else {
            return AcceptReturn::error();
        };

        let remote_endpoint = into_ipv6_socket_address(remote_endpoint);

        let Some(tcp_channels) = self.tcp_channels.upgrade() else {
            return AcceptReturn::error();
        };

        let result = {
            let mut guard = self.data.lock().unwrap();
            self.check_limits(remote_endpoint.ip(), direction, &mut guard)
        };

        if result != AcceptResult::Accepted {
            self.stats.inc_dir(
                StatType::TcpListener,
                DetailType::AcceptRejected,
                direction.into(),
            );
            debug!(
                "Rejected connection from: {} ({:?})",
                remote_endpoint, direction
            );
            // Rejection reason should be logged earlier

            if let Err(e) = raw_stream.shutdown().await {
                self.stats.inc_dir(
                    StatType::TcpListener,
                    DetailType::CloseError,
                    direction.into(),
                );
                debug!(
                    "Error while clsoing socket after refusing connection: {:?} ({:?})",
                    e, direction
                )
            }
            drop(raw_stream);
            return AcceptReturn::failed(result);
        }

        self.stats.inc_dir(
            StatType::TcpListener,
            DetailType::AcceptSuccess,
            direction.into(),
        );

        debug!("Accepted connection: {} ({:?})", remote_endpoint, direction);

        let socket_stats = Arc::new(SocketStats::new(Arc::clone(&self.stats)));
        let socket = SocketBuilder::new(
            direction.into(),
            Arc::clone(&self.workers),
            Arc::downgrade(&self.runtime),
        )
        .default_timeout(Duration::from_secs(
            self.node_config.tcp_io_timeout_s as u64,
        ))
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
        .use_existing_socket(raw_stream, remote_endpoint)
        .finish();

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
            &tcp_channels,
            Arc::clone(&socket),
            Arc::new(self.node_config.clone()),
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

        self.data.lock().unwrap().connections.push(Connection {
            endpoint: remote_endpoint,
            socket: Arc::downgrade(&socket),
            server: Arc::downgrade(&server),
        });

        socket.set_timeout(Duration::from_secs(
            self.network_params.network.idle_timeout_s as u64,
        ));

        socket.start();
        server.start();

        self.socket_observer.socket_connected(Arc::clone(&socket));

        AcceptReturn {
            result: AcceptResult::Accepted,
            socket: Some(socket),
            server: Some(server),
        }
    }
}

impl TcpServerObserver for TcpListener {
    fn bootstrap_server_timeout(&self, _connection_id: usize) {
        debug!("Closing TCP server due to timeout");
    }

    fn boostrap_server_exited(
        &self,
        socket_type: super::SocketType,
        _connection_id: usize,
        endpoint: SocketAddrV6,
    ) {
        debug!("Exiting server: {}", endpoint);
        if socket_type == super::SocketType::Bootstrap {
            self.bootstrap_count.fetch_sub(1, Ordering::SeqCst);
        } else if socket_type == super::SocketType::Realtime {
            self.realtime_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn bootstrap_count(&self) -> usize {
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

struct Attempt {
    endpoint: SocketAddrV6,
    start: Instant,
    join_handle: tokio::task::JoinHandle<()>,
}
