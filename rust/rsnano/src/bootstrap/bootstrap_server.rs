use std::{
    collections::VecDeque,
    net::{Ipv6Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    logger_mt::Logger,
    messages::Message,
    transport::{Socket, SocketImpl, SocketType},
    utils::ThreadPool,
    Account, NetworkFilter, NodeConfig,
};

pub trait BootstrapServerObserver {
    fn bootstrap_server_timeout(&self, inner_ptr: usize);
    fn boostrap_server_exited(
        &self,
        socket_type: SocketType,
        inner_ptr: usize,
        endpoint: SocketAddr,
    );
    fn get_bootstrap_count(&self) -> usize;
    fn inc_bootstrap_count(&self);
}

pub struct BootstrapServer {
    pub socket: Arc<SocketImpl>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
    stopped: AtomicBool,
    observer: Arc<dyn BootstrapServerObserver>,
    pub queue: Mutex<VecDeque<Option<Box<dyn Message>>>>,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,

    // Remote enpoint used to remove response channel even after socket closing
    pub remote_endpoint: Mutex<SocketAddr>,
    pub remote_node_id: Mutex<Account>,
    pub receive_buffer: Arc<Mutex<Vec<u8>>>,
    pub publish_filter: Arc<NetworkFilter>,
    pub workers: Arc<dyn ThreadPool>,
}

impl BootstrapServer {
    pub fn new(
        socket: Arc<SocketImpl>,
        config: Arc<NodeConfig>,
        logger: Arc<dyn Logger>,
        observer: Arc<dyn BootstrapServerObserver>,
        publish_filter: Arc<NetworkFilter>,
        workers: Arc<dyn ThreadPool>,
    ) -> Self {
        Self {
            socket,
            config,
            logger,
            observer,
            stopped: AtomicBool::new(false),
            queue: Mutex::new(VecDeque::new()),
            disable_bootstrap_listener: false,
            connections_max: 64,
            remote_endpoint: Mutex::new(SocketAddr::new(
                std::net::IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                0,
            )),
            remote_node_id: Mutex::new(Account::new()),
            receive_buffer: Arc::new(Mutex::new(vec![0; 1024])),
            publish_filter,
            workers,
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.socket.close();
        }
    }

    pub fn make_bootstrap_connection(&self) -> bool {
        if self.socket.socket_type() == SocketType::Undefined
            && !self.disable_bootstrap_listener
            && self.observer.get_bootstrap_count() < self.connections_max
        {
            self.observer.inc_bootstrap_count();
            self.socket.set_socket_type(SocketType::Bootstrap);
        }

        return self.socket.socket_type() == SocketType::Bootstrap;
    }
}

impl Drop for BootstrapServer {
    fn drop(&mut self) {
        self.stop();
    }
}
