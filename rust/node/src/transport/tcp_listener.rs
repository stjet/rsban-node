use super::{
    AcceptResult, ChannelDirection, ChannelMode, CompositeSocketObserver, Network,
    ResponseServerFactory, ResponseServerImpl, Socket, SocketBuilder, SocketObserver, TcpConfig,
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
        atomic::{AtomicU16, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

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

pub struct PeerConnector {}

/// Server side portion of tcp sessions. Listens for new socket connections and spawns tcp_server objects when connected.
pub struct TcpListener {
    port: AtomicU16,
    config: TcpConfig,
    node_config: NodeConfig,
    network: Arc<Network>,
    stats: Arc<Stats>,
    runtime: Arc<AsyncRuntime>,
    socket_observer: Arc<dyn SocketObserver>,
    workers: Arc<dyn ThreadPool>,
    network_params: NetworkParams,
    data: Mutex<TcpListenerData>,
    condition: Condvar,
    cancel_token: CancellationToken,
    response_server_factory: ResponseServerFactory,
    peer_connector: PeerConnector,
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
            peer_connector: PeerConnector {},
            port: AtomicU16::new(port),
            config,
            node_config,
            network,
            data: Mutex::new(TcpListenerData {
                stopped: false,
                local_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            }),
            network_params,
            runtime: Arc::clone(&runtime),
            socket_observer,
            stats,
            workers,
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
        self.network.count_by_mode(ChannelMode::Realtime)
    }

    pub fn connection_count(&self) -> usize {
        self.network.count_by_direction(ChannelDirection::Inbound)
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
    async fn connect(&self, remote: SocketAddrV6) -> bool;

    async fn connect_impl(&self, endpoint: SocketAddrV6) -> anyhow::Result<()>;
}

#[async_trait]
impl TcpListenerExt for Arc<TcpListener> {
    async fn connect(&self, remote: SocketAddrV6) -> bool {
        if self.is_stopped() {
            return false;
        }

        if !self.network.add_outbound_attempt(remote) {
            return false;
        }

        let self_l = Arc::clone(self);
        tokio::spawn(async move {
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

            self_l.network.remove_attempt(&remote);
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

            self_l.network.set_port(addr.port());
            self_l.data.lock().unwrap().local_addr = addr;

            self_l.run(listener).await
        });
    }

    async fn run(&self, listener: tokio::net::TcpListener) {
        let run_loop = async {
            loop {
                self.network.wait_for_available_inbound_slot().await;

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
                    .create_response_server(socket.clone());

                let _ = self
                    .network
                    .accept_one(&socket, &response_server, ChannelDirection::Inbound)
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
            .create_response_server(socket.clone());

        self.network
            .accept_one(&socket, &response_server, ChannelDirection::Outbound)
            .await
    }
}

fn is_temporary_error(ec: ErrorCode) -> bool {
    return ec.val == 11 // would block
                        || ec.val ==  4; // interrupted system call
}
