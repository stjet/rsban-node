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
    time::{Duration, Instant},
};

use crate::{
    logger_mt::Logger,
    messages::{
        BulkPull, BulkPullAccount, BulkPush, ConfirmAck, ConfirmReq, FrontierReq, Keepalive,
        Message, MessageHeader, MessageType, MessageVisitor, NodeIdHandshake, Publish,
        TelemetryAck, TelemetryReq,
    },
    stats::{DetailType, Direction, Stat, StatType},
    network::{Socket, SocketImpl, SocketType},
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
    publish_filter: Arc<NetworkFilter>,
    workers: Arc<dyn ThreadPool>,
    io_ctx: Arc<dyn IoContext>,

    network: NetworkParams,
    last_telemetry_req: Mutex<Option<Instant>>,
    unique_id: usize,
    stats: Arc<Stat>,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    pub handshake_query_received: AtomicBool,
    request_response_visitor_factory: Arc<dyn RequestResponseVisitorFactory>,
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
    fn requests_empty(&self) -> bool;
    fn push_request(&self, msg: Option<Box<dyn Message>>);
    fn run_next(&self, requests_lock: &BootstrapRequestsLock);
    fn receive(&self);
    fn finish_request(&self);
    fn finish_request_async(&self);
    fn add_request(&self, message: Box<dyn Message>);
    fn receive_header_action(&self, ec: ErrorCode, size: usize);
    fn receive_node_id_handshake_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_confirm_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_confirm_req_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_telemetry_ack_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_publish_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_keepalive_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_frontier_req_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_bulk_pull_account_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
    fn receive_bulk_pull_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader);
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
        // Increase timeout to receive TCP header (idle server socket)
        self.socket
            .set_default_timeout_value(self.network.network.idle_timeout_s as u64);
        let self_clone = self.clone();
        self.socket.async_read2(
            self.receive_buffer.clone(),
            8,
            Box::new(move |ec, size| {
                {
                    // Set remote_endpoint
                    let mut endpoint_lk = self_clone.remote_endpoint.lock().unwrap();
                    if endpoint_lk.port() == 0 {
                        if let Some(ep) = self_clone.socket.get_remote() {
                            *endpoint_lk = ep;
                        }
                    }
                }

                // Decrease timeout to default
                self_clone
                    .socket
                    .set_default_timeout_value(self_clone.config.tcp_io_timeout_s as u64);
                // Receive header
                self_clone.receive_header_action(ec, size);
            }),
        );
    }

    fn add_request(&self, message: Box<dyn Message>) {
        let lock = self.lock_requests();
        let start = lock.is_queue_empty();
        lock.push(Some(message));
        if start {
            self.run_next(&lock);
        }
    }

    fn receive_header_action(&self, ec: ErrorCode, size: usize) {
        if ec.is_ok() {
            debug_assert!(size == 8);
            let header = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                MessageHeader::from_stream(&mut stream)
            };

            match header {
                Ok(header) => {
                    if header.network() != self.network.network.current_network {
                        _ = self.stats.inc(
                            StatType::Message,
                            DetailType::InvalidNetwork,
                            Direction::In,
                        );
                        return;
                    }

                    if header.version_using() < self.network.network.protocol_version_min {
                        let _ = self.stats.inc(
                            StatType::Message,
                            DetailType::OutdatedVersion,
                            Direction::In,
                        );
                        return;
                    }

                    let self_clone = self.clone();
                    let buffer = self.receive_buffer.clone();

                    match header.message_type() {
                        MessageType::BulkPull => {
                            let _ = self.stats.inc(
                                StatType::Bootstrap,
                                DetailType::BulkPull,
                                Direction::In,
                            );
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_bulk_pull_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::BulkPullAccount => {
                            let _ = self.stats.inc(
                                StatType::Bootstrap,
                                DetailType::BulkPullAccount,
                                Direction::In,
                            );
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_bulk_pull_account_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::FrontierReq => {
                            let _ = self.stats.inc(
                                StatType::Bootstrap,
                                DetailType::FrontierReq,
                                Direction::In,
                            );
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_frontier_req_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::BulkPush => {
                            let _ = self.stats.inc(
                                StatType::Bootstrap,
                                DetailType::BulkPush,
                                Direction::In,
                            );
                            if self.make_bootstrap_connection() {
                                self.add_request(Box::new(BulkPush::with_header(&header)))
                            }
                        }
                        MessageType::Keepalive => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_keepalive_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::Publish => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_publish_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::ConfirmAck => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_confirm_ack_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::ConfirmReq => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_confirm_req_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::NodeIdHandshake => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_node_id_handshake_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::TelemetryReq => {
                            if self.socket.is_realtime_connection() {
                                // Only handle telemetry requests if they are outside of the cutoff time
                                let cache_exceeded = self.cache_exceeded();
                                if cache_exceeded {
                                    self.set_last_telemetry_req();
                                    self.add_request(Box::new(TelemetryReq::with_header(&header)));
                                } else {
                                    let _ = self.stats.inc(
                                        StatType::Telemetry,
                                        DetailType::RequestWithinProtectionCacheZone,
                                        Direction::In,
                                    );
                                }
                            }
                            self.receive();
                        }
                        MessageType::TelemetryAck => {
                            self.socket.async_read2(
                                buffer,
                                header.payload_length(),
                                Box::new(move |ec, size| {
                                    self_clone.receive_telemetry_ack_action(ec, size, &header);
                                }),
                            );
                        }
                        MessageType::Invalid | MessageType::NotAType => {
                            if self.config.logging.network_logging_value {
                                self.logger.try_log(&format!(
                                    "Received invalid type from bootstrap connection {}",
                                    header.message_type() as u8
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    if self.config.logging.network_logging_value {
                        self.logger.try_log(&format!(
                            "Received invalid type from bootstrap connection {}",
                            e
                        ));
                    }
                }
            }
        } else if self.config.logging.bulk_pull_logging_value {
            self.logger
                .try_log(&format!("Error while receiving type: {:?}", ec));
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

    fn receive_confirm_req_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                ConfirmReq::from_stream(&mut stream, header)
            };
            if let Ok(request) = request {
                if self.socket.is_realtime_connection() {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else if self.config.logging.network_message_logging_value {
            self.logger
                .try_log(&format!("Error receiving confirm_req: {:?}", ec));
        }
    }

    fn receive_publish_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let (digest, existed) = {
                let bytes = self.receive_buffer.lock().unwrap();
                self.publish_filter.apply(&bytes[..size])
            };

            if !existed {
                let request = {
                    let buffer = self.receive_buffer.lock().unwrap();
                    let mut stream = StreamAdapter::new(&buffer[..size]);
                    Publish::from_stream(&mut stream, header, digest)
                };

                if let Ok(request) = request {
                    if self.socket.is_realtime_connection() {
                        let insufficient_work = {
                            let block = request.block.as_ref().unwrap(); // block cannot be None after deserialize!
                            let lk = block.read().unwrap();
                            self.network.work.validate_entry_block(lk.as_block())
                        };
                        if !insufficient_work {
                            self.add_request(Box::new(request));
                        } else {
                            let _ = self.stats.inc_detail_only(
                                StatType::Error,
                                DetailType::InsufficientWork,
                                Direction::In,
                            );
                        }
                    }
                    self.receive();
                }
            } else {
                let _ = self.stats.inc(
                    StatType::Filter,
                    DetailType::DuplicatePublish,
                    Direction::In,
                );
                self.receive();
            }
        } else if self.config.logging.network_message_logging_value {
            self.logger
                .try_log(&format!("Error receiving publish: {:?}", ec));
        }
    }

    fn receive_keepalive_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                Keepalive::from_stream(header.clone(), &mut stream)
            };

            if let Ok(request) = request {
                if self.socket.is_realtime_connection() {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else if self.config.logging.network_message_logging_value {
            self.logger
                .try_log(&format!("Error receiving keepalive: {:?}", ec));
        }
    }

    fn receive_frontier_req_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                FrontierReq::from_stream(&mut stream, header)
            };

            if let Ok(request) = request {
                if self.config.logging.bulk_pull_logging_value {
                    self.logger.try_log(&format!(
                        "Received frontier request for {} with age {}",
                        request.start.encode_account(),
                        request.age
                    ));
                }
                if self.make_bootstrap_connection() {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        } else if self.config.logging.network_message_logging_value {
            self.logger
                .try_log(&format!("Error receiving frontier request: {:?}", ec));
        }
    }

    fn receive_bulk_pull_account_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                BulkPullAccount::from_stream(&mut stream, header)
            };

            if let Ok(request) = request {
                if self.config.logging.bulk_pull_logging_value {
                    self.logger.try_log(&format!(
                        "Received bulk pull account for {} with a minimum amount of {}",
                        request.account.encode_account(),
                        request.minimum_amount.format_balance(10)
                    ));
                }
                if self.make_bootstrap_connection() && !self.disable_bootstrap_bulk_pull_server {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        }
    }

    fn receive_bulk_pull_action(&self, ec: ErrorCode, size: usize, header: &MessageHeader) {
        if ec.is_ok() {
            let request = {
                let buffer = self.receive_buffer.lock().unwrap();
                let mut stream = StreamAdapter::new(&buffer[..size]);
                BulkPull::from_stream(&mut stream, header)
            };

            if let Ok(request) = request {
                if self.config.logging.bulk_pull_logging_value {
                    let remote = { self.remote_endpoint.lock().unwrap().clone() };
                    self.logger.try_log(&format!(
                        "Received bulk pull for {} down to {}, maximum of {} from {}",
                        request.start, request.end, request.count, remote
                    ));
                }
                if self.make_bootstrap_connection() && !self.disable_bootstrap_bulk_pull_server {
                    self.add_request(Box::new(request));
                }
                self.receive();
            }
        }
    }

    fn finish_request(&self) {
        let lock = self.lock_requests();
        if !lock.is_queue_empty() {
            lock.pop();
        } else {
            let _ = self.stats.inc(
                StatType::Bootstrap,
                DetailType::RequestUnderflow,
                Direction::In,
            );
        }

        while !lock.is_queue_empty() {
            if lock.front().is_none() {
                lock.pop();
            } else {
                self.run_next(&lock);
            }
        }

        let self_weak = Arc::downgrade(self);
        self.workers.add_timed_task(
            Duration::from_secs((self.config.tcp_io_timeout_s as u64 * 2) + 1),
            Box::new(move || {
                if let Some(self_clone) = self_weak.upgrade() {
                    self_clone.timeout();
                }
            }),
        );
    }

    fn finish_request_async(&self) {
        let self_weak = Arc::downgrade(self);
        self.io_ctx.post(Box::new(move || {
            if let Some(self_clone) = self_weak.upgrade() {
                self_clone.finish_request();
            }
        }));
    }

    fn requests_empty(&self) -> bool {
        self.lock_requests().is_queue_empty()
    }

    fn push_request(&self, msg: Option<Box<dyn Message>>) {
        self.lock_requests().push(msg)
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
