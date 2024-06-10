use super::{
    AcceptResult, CompositeSocketObserver, ConnectionDirection, ConnectionsPerAddress, Network,
    ResponseServer, ResponseServerFactory, ResponseServerObserver, Socket, SocketBuilder,
    SocketExtensions, SocketObserver, TcpConfig, TcpServerExt,
};
use crate::{
    config::NodeConfig,
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    utils::{into_ipv6_socket_address, AsyncRuntime, ErrorCode, ThreadPool},
    NetworkParams,
};
use async_trait::async_trait;
use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{
        atomic::{AtomicU16, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, Weak,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

pub struct AcceptReturn {
    result: AcceptResult,
    socket: Option<Arc<Socket>>,
    server: Option<Arc<ResponseServer>>,
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
    server: Weak<ResponseServer>,
}

/// Server side portion of tcp sessions. Listens for new socket connections and spawns tcp_server objects when connected.
pub struct TcpListener {
    port: AtomicU16,
    config: TcpConfig,
    node_config: NodeConfig,
    network: Weak<Network>,
    stats: Arc<Stats>,
    runtime: Arc<AsyncRuntime>,
    socket_observer: Arc<dyn SocketObserver>,
    workers: Arc<dyn ThreadPool>,
    network_params: NetworkParams,
    data: Mutex<TcpListenerData>,
    bootstrap_count: AtomicUsize,
    realtime_count: AtomicUsize,
    cleanup_thread: Mutex<Option<JoinHandle<()>>>,
    condition: Condvar,
    cancel_token: CancellationToken,
    response_server_factory: ResponseServerFactory,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        debug_assert!(self.cleanup_thread.lock().unwrap().is_none());
        debug_assert!(self.data.lock().unwrap().stopped);
        debug_assert_eq!(self.connection_count(), 0);
    }
}

struct TcpListenerData {
    connections: Vec<Connection>,
    stopped: bool,
    local_addr: SocketAddr,
}

struct ServerSocket {
    socket: Arc<Socket>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
}

impl TcpListenerData {
    fn alive_connections_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.socket.strong_count() > 0)
            .count()
    }
}

impl TcpListener {
    pub(crate) fn new(
        port: u16,
        config: TcpConfig,
        node_config: NodeConfig,
        network: Arc<Network>,
        network_params: NetworkParams,
        runtime: Arc<AsyncRuntime>,
        socket_observer: Arc<dyn SocketObserver>,
        stats: Arc<Stats>,
        workers: Arc<dyn ThreadPool>,
        response_server_factory: ResponseServerFactory,
    ) -> Self {
        Self {
            port: AtomicU16::new(port),
            config,
            node_config,
            network: Arc::downgrade(&network),
            data: Mutex::new(TcpListenerData {
                connections: Vec::new(),
                stopped: false,
                local_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            }),
            network_params,
            runtime: Arc::clone(&runtime),
            socket_observer,
            stats,
            workers,
            bootstrap_count: AtomicUsize::new(0),
            realtime_count: AtomicUsize::new(0),
            cleanup_thread: Mutex::new(None),
            condition: Condvar::new(),
            cancel_token: CancellationToken::new(),
            response_server_factory,
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

        self.cancel_token.cancel();
        self.condition.notify_all();

        if let Some(handle) = self.cleanup_thread.lock().unwrap().take() {
            handle.join().unwrap();
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
        let data = self.data.lock().unwrap();
        data.alive_connections_count()
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

    fn is_stopped(&self) -> bool {
        self.data.lock().unwrap().stopped
    }
}

#[async_trait]
pub trait TcpListenerExt {
    fn start(&self);
    async fn run(&self, listener: tokio::net::TcpListener);
    fn connect_ip(&self, remote: Ipv6Addr) -> bool;
    fn connect(&self, remote: SocketAddrV6) -> bool;
    fn as_observer(self) -> Arc<dyn ResponseServerObserver>;

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
        if self.is_stopped() {
            return false;
        }

        let Some(channels) = self.network.upgrade() else {
            return false;
        };

        if !channels.add_inbound_attempt(remote) {
            return false;
        }

        let self_l = Arc::clone(self);
        self.runtime.tokio.spawn(async move {
            tokio::select! {
                result =  self_l.connect_impl(remote) =>{
                    if let Err(e) = result {
                        self_l.stats.inc_dir(
                            StatType::TcpListener,
                            DetailType::ConnectError,
                            Direction::Out,
                        );
                        debug!("Error connecting to: {} ({:?})", remote, e);
                    }

                },
                _ = tokio::time::sleep(self_l.config.connect_timeout) =>{
                    self_l.stats
                        .inc(StatType::TcpListener, DetailType::AttemptTimeout);
                    debug!(
                        "Connection attempt timed out: {}",
                        remote,
                    );

                }
                _ = self_l.cancel_token.cancelled() =>{
                    debug!(
                        "Connection attempt cancelled: {}",
                        remote,
                    );

                }
            }

            if let Some(channels) = self_l.network.upgrade() {
                channels.remove_attempt(&remote);
            }
        });

        true // Attempt started
    }

    fn start(&self) {
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

            if let Some(channels) = self_l.network.upgrade() {
                channels.set_port(addr.port());
            }
            self_l.data.lock().unwrap().local_addr = addr;

            self_l.run(listener).await
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

    async fn run(&self, listener: tokio::net::TcpListener) {
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
            _ = self.cancel_token.cancelled() => { },
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

    fn as_observer(self) -> Arc<dyn ResponseServerObserver> {
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

        let Some(tcp_channels) = self.network.upgrade() else {
            return AcceptReturn::error();
        };

        let result = tcp_channels.check_limits(remote_endpoint.ip(), direction);

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

        let server = self
            .response_server_factory
            .create_response_server(socket.clone(), &Arc::clone(self).as_observer());

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

impl ResponseServerObserver for TcpListener {
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
