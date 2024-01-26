use super::{
    ConnectionsPerAddress, Socket, SocketObserver, TcpSocketFacadeFactory, TokioSocketFacade,
};
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::Stats,
    utils::{AsyncRuntime, ThreadPool},
    NetworkParams,
};
use rsnano_core::utils::Logger;
use std::sync::{Arc, Mutex, Weak};

pub struct ServerSocket {
    pub socket: Arc<Socket>,
    pub socket_facade: Arc<TokioSocketFacade>,
    pub connections_per_address: Mutex<ConnectionsPerAddress>,
    pub node_flags: NodeFlags,
    pub network_params: NetworkParams,
    pub workers: Arc<dyn ThreadPool>,
    pub logger: Arc<dyn Logger>,
    pub tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    pub node_config: NodeConfig,
    pub stats: Arc<Stats>,
    pub socket_observer: Weak<dyn SocketObserver>,
    pub max_inbound_connections: usize,
    pub runtime: Weak<AsyncRuntime>,
}
