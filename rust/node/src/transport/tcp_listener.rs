use super::{
    ServerSocket, ServerSocketExtensions, Socket, SocketObserver, SynCookies, TcpChannels,
    TcpServer, TcpServerExt, TcpSocketFacadeFactory, TokioSocketFacade, TokioSocketFacadeFactory,
};
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::Stats,
    utils::{AsyncRuntime, ErrorCode, ThreadPool},
    NetworkParams,
};
use rsnano_core::utils::Logger;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{Arc, Weak},
};

pub struct TcpListener {
    port: u16,
    max_inbound_connections: usize,
    config: NodeConfig,
    logger: Arc<dyn Logger>,
    tcp_channels: Arc<TcpChannels>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    data: TcpListenerData,
    runtime: Weak<AsyncRuntime>,
    socket_observer: Arc<dyn SocketObserver>,
    workers: Arc<dyn ThreadPool>,
    tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    network_params: NetworkParams,
    node_flags: NodeFlags,
    socket_facade: Arc<TokioSocketFacade>,
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
            data: TcpListenerData {
                connections: HashMap::new(),
                on: false,
                listening_socket: None,
            },
            network_params,
            node_flags,
            runtime: Arc::downgrade(&runtime),
            socket_facade: Arc::new(TokioSocketFacade::create(runtime)),
            socket_observer,
            tcp_socket_facade_factory,
            stats,
            workers,
        }
    }

    pub fn start(
        &mut self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) -> anyhow::Result<()> {
        self.data.on = true;
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
            Weak::clone(&self.runtime),
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
        self.data.listening_socket = Some(listening_socket);
        Ok(())
    }

    pub fn add_connection(&mut self, conn: &Arc<TcpServer>) {
        self.data
            .connections
            .insert(conn.unique_id(), Arc::downgrade(conn));
        conn.start();
    }

    pub fn remove_connection(&mut self, connection_id: usize) {
        self.data.connections.remove(&connection_id);
    }

    pub fn connection_count(&self) -> usize {
        self.data.connections.len()
    }

    pub fn clear_connections(&mut self) {
        // TODO swap with lock and then clear after lock dropped
        self.data.connections.clear();
    }

    pub fn is_on(&self) -> bool {
        self.data.on
    }

    pub fn set_on(&mut self) {
        self.data.on = true;
    }

    pub fn set_off(&mut self) {
        self.data.on = false;
    }

    pub fn set_listening_socket(&mut self, socket: Arc<ServerSocket>) {
        self.data.listening_socket = Some(socket);
    }

    pub fn close_listening_socket(&mut self) {
        if let Some(socket) = self.data.listening_socket.take() {
            socket.close();
        }
    }

    pub fn has_listening_socket(&self) -> bool {
        self.data.listening_socket.is_some()
    }
}
