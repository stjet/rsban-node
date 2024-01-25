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
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex, Weak},
};

pub struct TcpListener {
    port: u16,
    max_inbound_connections: usize,
    config: NodeConfig,
    logger: Arc<dyn Logger>,
    tcp_channels: Arc<TcpChannels>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    runtime: Weak<AsyncRuntime>,
    socket_observer: Arc<dyn SocketObserver>,
    workers: Arc<dyn ThreadPool>,
    tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    network_params: NetworkParams,
    node_flags: NodeFlags,
    socket_facade: Arc<TokioSocketFacade>,
    data: Mutex<TcpListenerData>,
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
            data: Mutex::new(TcpListenerData {
                connections: HashMap::new(),
                on: false,
                listening_socket: None,
            }),
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
        data.listening_socket = Some(listening_socket);
        Ok(())
    }

    pub fn stop(&mut self) {
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

    pub fn add_connection(&mut self, conn: &Arc<TcpServer>) {
        let mut data = self.data.lock().unwrap();
        data.connections
            .insert(conn.unique_id(), Arc::downgrade(conn));
        conn.start();
    }

    pub fn remove_connection(&mut self, connection_id: usize) {
        let mut data = self.data.lock().unwrap();
        data.connections.remove(&connection_id);
    }
}
