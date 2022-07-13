use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_void,
    net::{Ipv6Addr, SocketAddr},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::Instant,
};

use crate::{
    logger_mt::Logger,
    messages::{
        ConfirmAck, Message, MessageHeader, MessageType, MessageVisitor, NodeIdHandshake,
        TelemetryAck,
    },
    stats::Stat,
    transport::{Socket, SocketImpl, SocketType},
    utils::{ErrorCode, IoContext, StreamAdapter, ThreadPool},
    Account, NetworkFilter, NetworkParams, NodeConfig, TelemetryCacheCutoffs,
};

pub trait BootstrapServerObserver {
    fn bootstrap_server_timeout(&self, inner_ptr: usize);
    fn boostrap_server_exited(
        &self,
        socket_type: SocketType,
        unique_id: usize,
        endpoint: SocketAddr,
    );
    fn get_bootstrap_count(&self) -> usize;
    fn inc_bootstrap_count(&self);
}

pub struct BootstrapServer {
    pub socket: Arc<SocketImpl>,
    pub config: Arc<NodeConfig>,
    pub logger: Arc<dyn Logger>,
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
    pub io_ctx: Arc<dyn IoContext>,

    pub network: NetworkParams,
    last_telemetry_req: Mutex<Option<Instant>>,
    unique_id: usize,
    pub stats: Arc<Stat>,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    pub handshake_query_received: AtomicBool,
    pub request_response_visitor_factory: Arc<dyn RequestResponseVisitorFactory>,
}

static NEXT_UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);

impl BootstrapServer {
    pub fn new(
        socket: Arc<SocketImpl>,
        config: Arc<NodeConfig>,
        logger: Arc<dyn Logger>,
        observer: Arc<dyn BootstrapServerObserver>,
        publish_filter: Arc<NetworkFilter>,
        workers: Arc<dyn ThreadPool>,
        io_ctx: Arc<dyn IoContext>,
        network: NetworkParams,
        stats: Arc<Stat>,
        request_response_visitor_factory: Arc<dyn RequestResponseVisitorFactory>,
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
            io_ctx,
            last_telemetry_req: Mutex::new(None),
            network,
            unique_id: NEXT_UNIQUE_ID.fetch_add(1, Ordering::Relaxed),
            stats,
            disable_bootstrap_bulk_pull_server: false,
            disable_tcp_realtime: false,
            handshake_query_received: AtomicBool::new(false),
            request_response_visitor_factory,
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

    pub fn set_last_telemetry_req(&self) {
        let mut lk = self.last_telemetry_req.lock().unwrap();
        *lk = Some(Instant::now());
    }

    pub fn cache_exceeded(&self) -> bool {
        let lk = self.last_telemetry_req.lock().unwrap();
        if let Some(last_req) = lk.as_ref() {
            last_req.elapsed() >= TelemetryCacheCutoffs::network_to_time(&self.network.network)
        } else {
            true
        }
    }

    pub fn unique_id(&self) -> usize {
        self.unique_id
    }
}

impl Drop for BootstrapServer {
    fn drop(&mut self) {
        let remote_ep = { self.remote_endpoint.lock().unwrap().clone() };
        self.observer.boostrap_server_exited(
            self.socket.socket_type(),
            self.unique_id(),
            remote_ep,
        );
        self.stop();
    }
}

pub trait RequestResponseVisitorFactory {
    fn create_visitor(
        &self,
        connection: &Arc<BootstrapServer>,
        requests_lock: &BootstrapRequestsLock,
    ) -> Box<dyn MessageVisitor>;

    fn handle(&self) -> *mut c_void;
}

pub trait BootstrapServerExt {
    fn timeout(&self);
    fn lock_requests(&self) -> BootstrapRequestsLock;
    fn run_next(&self, requests_lock: &BootstrapRequestsLock);
    fn receive(&self);
    fn add_request(&self, message: Box<dyn Message>);
    fn receive_node_id_handshake_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_confirm_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_telemetry_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
}

impl BootstrapServerExt for Arc<BootstrapServer> {
    fn timeout(&self) {
        if self.socket.has_timed_out() {
            self.observer.bootstrap_server_timeout(self.unique_id());
            self.socket.close();
        }
    }

    fn lock_requests(&self) -> BootstrapRequestsLock {
        let guard = self.queue.lock().unwrap();
        BootstrapRequestsLock::new(Arc::clone(self), guard)
    }

    fn run_next(&self, requests_lock: &BootstrapRequestsLock) {
        debug_assert!(!requests_lock.is_queue_empty());
        let visitor = self
            .request_response_visitor_factory
            .create_visitor(self, requests_lock);
        let msg_type = requests_lock.front().unwrap().header().message_type();
        if msg_type == MessageType::BulkPull
            || msg_type == MessageType::BulkPullAccount
            || msg_type == MessageType::BulkPush
            || msg_type == MessageType::FrontierReq
            || msg_type == MessageType::NodeIdHandshake
        {
            // Bootstrap & node ID (realtime start)
            // Request removed from queue in request_response_visitor. For bootstrap with requests.front ().release (), for node ID with finish_request ()
            if let Some(msg) = requests_lock.front() {
                msg.visit(visitor.as_ref())
            }
        } else {
            // Realtime
            if let Some(msg) = requests_lock.front() {
                requests_lock.pop();
                requests_lock.unlock();
                msg.visit(visitor.as_ref());
                requests_lock.relock();
            }
        }
    }

    fn receive(&self) {
        crate::ffi::bootstrap::bootstrap_server_receive(Arc::clone(self))
    }

    fn add_request(&self, message: Box<dyn Message>) {
        let lock = self.lock_requests();
        let start = lock.is_queue_empty();
        lock.push(Some(message));
        if start {
            self.run_next(&lock);
        }
    }

    fn receive_node_id_handshake_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                NodeIdHandshake::from_stream(&mut stream, header)
            };

            if let Ok(request) = request {
                if self.socket.socket_type() == SocketType::Undefined && !self.disable_tcp_realtime
                {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else if self.config.logging.network_node_id_handshake_logging_value {
            self.logger
                .try_log(&format!("Error receiving node_id_handshake: {:?}", ec));
        }
    }

    fn receive_confirm_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                ConfirmAck::with_header(header, &mut stream, None)
            };

            if let Ok(request) = request {
                if self.socket.is_realtime_connection() {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else if self.config.logging.network_message_logging_value {
            self.logger
                .try_log(&format!("Error receiving confirm_ack: {:?}", ec));
        }
    }

    fn receive_telemetry_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                TelemetryAck::from_stream(&mut stream, header)
            };

            if let Ok(request) = request {
                if self.socket.is_realtime_connection() {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else {
            if self.config.logging.network_telemetry_logging_value {
                self.logger
                    .try_log(&format!("Error receiving telemetry ack: {:?}", ec));
            }
        }
    }
}

#[derive(Clone)]
pub struct BootstrapRequestsLock {
    server: Arc<BootstrapServer>,
    requests: Rc<RefCell<Option<MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>>>>,
}

impl BootstrapRequestsLock {
    pub fn new(
        server: Arc<BootstrapServer>,
        guard: MutexGuard<VecDeque<Option<Box<dyn Message>>>>,
    ) -> Self {
        let guard = unsafe {
            std::mem::transmute::<
                MutexGuard<VecDeque<Option<Box<dyn Message>>>>,
                MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>,
            >(guard)
        };
        Self {
            server,
            requests: Rc::new(RefCell::new(Some(guard))),
        }
    }

    pub fn unlock(&self) {
        let mut inner = self.requests.borrow_mut();
        *inner = None;
    }

    pub fn relock(&self) {
        let guard = self.server.queue.lock().unwrap();
        let mut inner = self.requests.borrow_mut();
        *inner = unsafe {
            Some(std::mem::transmute::<
                MutexGuard<VecDeque<Option<Box<dyn Message>>>>,
                MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>,
            >(guard))
        };
    }

    pub fn release_front_request(&self) -> Option<Box<dyn Message>> {
        let mut requests = self.requests.borrow_mut();
        if let Some(r) = requests.as_mut() {
            if let Some(req) = r.front_mut() {
                return req.take();
            }
        }

        None
    }

    pub fn is_queue_empty(&self) -> bool {
        let requests = self.requests.borrow();
        if let Some(r) = requests.as_ref() {
            r.is_empty()
        } else {
            true
        }
    }

    pub fn front(&self) -> Option<Box<dyn Message>> {
        let requests = self.requests.borrow();
        if let Some(r) = requests.as_ref() {
            if let Some(req) = r.front() {
                if let Some(msg) = req {
                    return Some(msg.clone_box());
                }
            }
        }

        None
    }

    pub fn pop(&self) {
        let mut requests = self.requests.borrow_mut();
        if let Some(r) = requests.as_mut() {
            r.pop_front();
        }
    }

    pub fn push(&self, msg: Option<Box<dyn Message>>) {
        let mut requests = self.requests.borrow_mut();
        if let Some(r) = requests.as_mut() {
            r.push_back(msg)
        }
    }
}
