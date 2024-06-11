use super::{
    AcceptResult, ChannelDirection, CompositeSocketObserver, ConnectionsPerAddress, Network,
    ResponseServerFactory, ResponseServerImpl, ResponseServerObserver, Socket, SocketBuilder,
    SocketObserver, TcpConfig,
};
use crate::{
    config::NodeConfig,
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    transport::TcpStream,
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
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

pub struct AcceptReturn {
    result: AcceptResult,
    socket: Option<Arc<Socket>>,
    server: Option<Arc<ResponseServerImpl>>,
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
    condition: Condvar,
    cancel_token: CancellationToken,
    response_server_factory: ResponseServerFactory,
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        debug_assert!(self.data.lock().unwrap().stopped);
        debug_assert_eq!(self.connection_count(), 0);
    }
}

struct TcpListenerData {
    stopped: bool,
    local_addr: SocketAddr,
}

struct ServerSocket {
    socket: Arc<Socket>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
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
            condition: Condvar::new(),
            cancel_token: CancellationToken::new(),
            response_server_factory,
        }
    }

    pub fn stop(&self) {
        self.data.lock().unwrap().stopped = true;
        self.cancel_token.cancel();
        self.condition.notify_all();
    }

    pub fn realtime_count(&self) -> usize {
        self.realtime_count.load(Ordering::SeqCst)
    }

    pub fn connection_count(&self) -> usize {
        let Some(network) = self.network.upgrade() else {
            return 0;
        };

        network.count_by_direction(ChannelDirection::Inbound)
    }

    pub fn local_address(&self) -> SocketAddr {
        let guard = self.data.lock().unwrap();
        if !guard.stopped {
            guard.local_addr
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0)
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
        socket: Arc<Socket>,
        response_server: Arc<ResponseServerImpl>,
        direction: ChannelDirection,
    ) -> anyhow::Result<()>;

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

        if !channels.add_outbound_attempt(remote) {
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

            if let Some(network) = self_l.network.upgrade() {
                network.remove_attempt(&remote);
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

                let raw_stream = TcpStream::new(stream);

                let Ok(remote_endpoint) = raw_stream.peer_addr() else {
                    continue;
                };

                let remote_endpoint = into_ipv6_socket_address(remote_endpoint);
                let socket_stats = Arc::new(SocketStats::new(Arc::clone(&self.stats)));
                let socket = SocketBuilder::new(
                    ChannelDirection::Inbound,
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

                let response_server = self
                    .response_server_factory
                    .create_response_server(socket.clone(), &Arc::clone(self).as_observer());

                let _ = self
                    .accept_one(socket, response_server, ChannelDirection::Inbound)
                    .await;

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
        let raw_stream = TcpStream::new(raw_stream);
        let remote_endpoint = raw_stream.peer_addr()?;

        let remote_endpoint = into_ipv6_socket_address(remote_endpoint);
        let socket_stats = Arc::new(SocketStats::new(Arc::clone(&self.stats)));
        let socket = SocketBuilder::new(
            ChannelDirection::Outbound,
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

        let response_server = self
            .response_server_factory
            .create_response_server(socket.clone(), &Arc::clone(self).as_observer());

        self.accept_one(socket, response_server, ChannelDirection::Outbound)
            .await
    }

    async fn accept_one(
        &self,
        socket: Arc<Socket>,
        response_server: Arc<ResponseServerImpl>,
        direction: ChannelDirection,
    ) -> anyhow::Result<()> {
        let Some(network) = self.network.upgrade() else {
            return Err(anyhow!("No network"));
        };

        network
            .accept_one(&socket, &response_server, direction)
            .await?;

        Ok(())
    }
}

impl ResponseServerObserver for TcpListener {
    fn bootstrap_server_timeout(&self, _connection_id: usize) {
        debug!("Closing TCP server due to timeout");
    }

    fn boostrap_server_exited(
        &self,
        socket_type: super::ChannelMode,
        _connection_id: usize,
        endpoint: SocketAddrV6,
    ) {
        debug!("Exiting server: {}", endpoint);
        if socket_type == super::ChannelMode::Bootstrap {
            self.bootstrap_count.fetch_sub(1, Ordering::SeqCst);
        } else if socket_type == super::ChannelMode::Realtime {
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
