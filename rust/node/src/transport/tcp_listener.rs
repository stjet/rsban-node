use super::{
    ServerSocket, ServerSocketExtensions, Socket, SocketExtensions, SocketObserver, SynCookies,
    TcpChannels, TcpMessageManager, TcpServer, TcpServerExt, TcpServerObserver,
    TcpSocketFacadeFactory, TokioSocketFacade, TokioSocketFacadeFactory,
};
use crate::{
    block_processing::BlockProcessor,
    bootstrap::{BootstrapInitiator, BootstrapMessageVisitorFactory},
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
    utils::{AsyncRuntime, ErrorCode, ThreadPool},
    NetworkParams,
};
use rsnano_core::{utils::Logger, KeyPair};
use rsnano_ledger::Ledger;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};

pub struct TcpListener {
    port: u16,
    max_inbound_connections: usize,
    config: NodeConfig,
    logger: Arc<dyn Logger>,
    tcp_channels: Arc<TcpChannels>,
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

struct TcpListenerData {
    connections: HashMap<usize, Weak<TcpServer>>,
    on: bool,
    listening_socket: Option<Arc<ServerSocket>>, // TODO remove arc
}

impl TcpListener {
    pub fn new(
        port: u16,
        max_inbound_connections: usize,
        config: NodeConfig,
        logger: Arc<dyn Logger>,
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
            port,
            max_inbound_connections,
            config,
            logger,
            tcp_channels,
            syn_cookies,
            data: Mutex::new(TcpListenerData {
                connections: HashMap::new(),
                on: false,
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

    pub fn start(
        &mut self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock().unwrap();
        data.on = true;
        let listening_socket = Arc::new(ServerSocket::new(
            Arc::clone(&self.socket_facade),
            self.node_flags.clone(),
            self.network_params.clone(),
            Arc::clone(&self.workers),
            Arc::clone(&self.logger),
            Arc::clone(&self.tcp_socket_facade_factory),
            self.config.clone(),
            Arc::clone(&self.stats),
            Arc::clone(&self.socket_observer),
            self.max_inbound_connections,
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), self.port),
            Arc::downgrade(&self.runtime),
        ));
        let ec = listening_socket.start();
        if ec.is_err() {
            self.logger.always_log(&format!(
                "Network: Error while binding for incoming TCP/bootstrap on port {}: {:?}",
                listening_socket.listening_port(),
                ec
            ));
            bail!("Network: Error while binding for incoming TCP/bootstrap");
        }

        // the user can either specify a port value in the config or it can leave the choice up to the OS:
        // (1): port specified
        // (2): port not specified
        let listening_port = listening_socket.listening_port();

        // (1) -- nothing to do
        //
        if self.port == listening_port {
        }
        // (2) -- OS port choice happened at TCP socket bind time, so propagate this port value back;
        // the propagation is done here for the `tcp_listener` itself, whereas for `network`, the node does it
        // after calling `tcp_listener.start ()`
        //
        else {
            self.port = listening_port;
        }

        listening_socket.on_connection(callback);
        data.listening_socket = Some(listening_socket);
        Ok(())
    }

    pub fn stop(&self) {
        let mut conns = HashMap::new();
        {
            let mut guard = self.data.lock().unwrap();
            guard.on = false;
            std::mem::swap(&mut conns, &mut guard.connections);

            if let Some(socket) = guard.listening_socket.take() {
                socket.close();
            }
        }
    }

    pub fn connection_count(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.connections.len()
    }

    pub fn endpoint(&self) -> SocketAddrV6 {
        let guard = self.data.lock().unwrap();
        if guard.on && guard.listening_socket.is_some() {
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port, 0, 0)
        } else {
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)
        }
    }

    pub fn add_connection(&self, conn: &Arc<TcpServer>) {
        let mut data = self.data.lock().unwrap();
        data.connections
            .insert(conn.unique_id(), Arc::downgrade(conn));
        conn.start();
    }

    pub fn remove_connection(&self, connection_id: usize) {
        let mut data = self.data.lock().unwrap();
        data.connections.remove(&connection_id);
    }
}

pub trait TcpListenerExt {
    fn accept_action(&self, ec: ErrorCode, socket: Arc<Socket>);
}

impl TcpListenerExt for Arc<TcpListener> {
    fn accept_action(&self, ec: ErrorCode, socket: Arc<Socket>) {
        let Some(remote) = socket.get_remote() else {
            return;
        };
        if !self
            .tcp_channels
            .excluded_peers
            .lock()
            .unwrap()
            .is_excluded(&remote)
        {
            let message_visitor_factory = Arc::new(BootstrapMessageVisitorFactory::new(
                Arc::clone(&self.runtime),
                Arc::clone(&self.logger),
                Arc::clone(&self.syn_cookies),
                Arc::clone(&self.stats),
                self.network_params.network.clone(),
                Arc::clone(&self.node_id),
                Arc::clone(&self.ledger),
                Arc::clone(&self.workers),
                Arc::clone(&self.block_processor),
                Arc::clone(&self.bootstrap_initiator),
                self.node_flags.clone(),
                self.config.logging.clone(),
            ));
            let observer = Arc::clone(&self);
            let server = Arc::new(TcpServer::new(
                Arc::clone(&self.runtime),
                socket,
                Arc::new(self.config.clone()),
                Arc::clone(&self.logger),
                observer,
                Arc::clone(&self.tcp_channels.publish_filter),
                Arc::new(self.network_params.clone()),
                Arc::clone(&self.stats),
                Arc::clone(&self.tcp_channels.tcp_message_manager),
                message_visitor_factory,
                true,
            ));

            let mut data = self.data.lock().unwrap();
            data.connections
                .insert(server.unique_id(), Arc::downgrade(&server));
            server.start();
        } else {
            self.stats
                .inc(StatType::Tcp, DetailType::TcpExcluded, Direction::In);
            if self.config.logging.network_rejected_logging() {
                self.logger.try_log(&format!(
                    "Rejected connection from excluded peer {}",
                    remote
                ));
            }
        }
    }
}

impl TcpServerObserver for TcpListener {
    fn bootstrap_server_timeout(&self, connection_id: usize) {
        if self.config.logging.bulk_pull_logging() {
            self.logger
                .try_log("Closing incoming tcp / bootstrap server by timeout");
        }
        self.remove_connection(connection_id)
    }

    fn boostrap_server_exited(
        &self,
        socket_type: super::SocketType,
        connection_id: usize,
        endpoint: SocketAddrV6,
    ) {
        if self.config.logging.bulk_pull_logging() {
            self.logger.try_log("Exiting incoming TCP/bootstrap server");
        }
        if socket_type == super::SocketType::Bootstrap {
            self.bootstrap_count.fetch_sub(1, Ordering::SeqCst);
        } else if socket_type == super::SocketType::Realtime {
            self.realtime_count.fetch_sub(1, Ordering::SeqCst);
            // Clear temporary channel
            self.tcp_channels.erase_temporary_channel(&endpoint);
        }
        self.remove_connection(connection_id);
    }

    fn get_bootstrap_count(&self) -> usize {
        self.bootstrap_count.load(Ordering::SeqCst)
    }

    fn inc_bootstrap_count(&self) {
        self.bootstrap_count.fetch_add(1, Ordering::SeqCst);
    }

    fn inc_realtime_count(&self) {
        self.realtime_count.fetch_add(1, Ordering::SeqCst);
    }
}
