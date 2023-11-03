use std::{
    net::{Ipv6Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::{Logger, MemoryStream},
    Account, KeyPair,
};
use tokio::task::spawn_blocking;

use crate::{
    bootstrap::BootstrapMessageVisitorFactory,
    config::{NetworkConstants, NodeConfig},
    messages::{
        Message, MessageEnum, MessageVisitor, NodeIdHandshake, NodeIdHandshakeQuery,
        NodeIdHandshakeResponse, Payload, TelemetryAck, TelemetryReq,
    },
    stats::{DetailType, Direction, StatType, Stats},
    transport::{
        ParseStatus, Socket, SocketExtensions, SocketType, SynCookies, TcpMessageItem,
        TcpMessageManager,
    },
    utils::{AsyncRuntime, BlockUniquer},
    voting::VoteUniquer,
    NetworkParams,
};

use super::{AsyncMessageDeserializer, NetworkFilter};

pub trait TcpServerObserver: Send + Sync {
    fn bootstrap_server_timeout(&self, inner_ptr: usize);
    fn boostrap_server_exited(
        &self,
        socket_type: SocketType,
        unique_id: usize,
        endpoint: SocketAddr,
    );
    fn get_bootstrap_count(&self) -> usize;
    fn inc_bootstrap_count(&self);
    fn inc_realtime_count(&self);
}

pub struct NullTcpServerObserver {}
impl TcpServerObserver for NullTcpServerObserver {
    fn bootstrap_server_timeout(&self, _inner_ptr: usize) {}

    fn boostrap_server_exited(
        &self,
        _socket_type: SocketType,
        _unique_id: usize,
        _endpoint: SocketAddr,
    ) {
    }

    fn get_bootstrap_count(&self) -> usize {
        0
    }

    fn inc_bootstrap_count(&self) {}

    fn inc_realtime_count(&self) {}
}

pub struct TcpServer {
    async_rt: Arc<AsyncRuntime>,
    pub socket: Arc<Socket>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
    stopped: AtomicBool,
    observer: Arc<dyn TcpServerObserver>,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,

    // Remote enpoint used to remove response channel even after socket closing
    remote_endpoint: Mutex<SocketAddr>,
    pub remote_node_id: Mutex<Account>,

    network: Arc<NetworkParams>,
    last_telemetry_req: Mutex<Option<Instant>>,
    unique_id: usize,
    stats: Arc<Stats>,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    handshake_query_received: AtomicBool,
    message_visitor_factory: Arc<BootstrapMessageVisitorFactory>,
    message_deserializer: Arc<AsyncMessageDeserializer<Arc<Socket>>>,
    tcp_message_manager: Arc<TcpMessageManager>,
    allow_bootstrap: bool,
}

static NEXT_UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);

impl TcpServer {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        socket: Arc<Socket>,
        config: Arc<NodeConfig>,
        logger: Arc<dyn Logger>,
        observer: Arc<dyn TcpServerObserver>,
        publish_filter: Arc<NetworkFilter>,
        network: Arc<NetworkParams>,
        stats: Arc<Stats>,
        block_uniquer: Arc<BlockUniquer>,
        vote_uniquer: Arc<VoteUniquer>,
        tcp_message_manager: Arc<TcpMessageManager>,
        message_visitor_factory: Arc<BootstrapMessageVisitorFactory>,
        allow_bootstrap: bool,
    ) -> Self {
        let network_constants = network.network.clone();
        let socket_clone = Arc::clone(&socket);
        Self {
            async_rt,
            socket,
            config,
            logger,
            observer,
            stopped: AtomicBool::new(false),
            disable_bootstrap_listener: false,
            connections_max: 64,
            remote_endpoint: Mutex::new(SocketAddr::new(
                std::net::IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                0,
            )),
            remote_node_id: Mutex::new(Account::zero()),
            last_telemetry_req: Mutex::new(None),
            network,
            unique_id: NEXT_UNIQUE_ID.fetch_add(1, Ordering::Relaxed),
            stats,
            disable_bootstrap_bulk_pull_server: false,
            disable_tcp_realtime: false,
            handshake_query_received: AtomicBool::new(false),
            message_visitor_factory,
            message_deserializer: Arc::new(AsyncMessageDeserializer::new(
                network_constants,
                publish_filter,
                block_uniquer,
                vote_uniquer,
                socket_clone,
            )),
            tcp_message_manager,
            allow_bootstrap,
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

    pub fn was_handshake_query_received(&self) -> bool {
        self.handshake_query_received.load(Ordering::SeqCst)
    }

    pub fn handshake_query_received(&self) {
        self.handshake_query_received.store(true, Ordering::SeqCst);
    }

    pub fn remote_endpoint(&self) -> SocketAddr {
        *self.remote_endpoint.lock().unwrap()
    }

    pub fn is_outside_cooldown_period(&self) -> bool {
        let lock = self.last_telemetry_req.lock().unwrap();
        match *lock {
            Some(last_req) => {
                last_req.elapsed()
                    >= Duration::from_millis(
                        self.network.network.telemetry_request_cooldown_ms as u64,
                    )
            }
            None => true,
        }
    }

    pub fn to_bootstrap_connection(&self) -> bool {
        if !self.allow_bootstrap {
            return false;
        }

        if self.disable_bootstrap_listener {
            return false;
        }

        if self.observer.get_bootstrap_count() >= self.connections_max {
            return false;
        }

        if self.socket.socket_type() != SocketType::Undefined {
            return false;
        }

        self.observer.inc_bootstrap_count();
        self.socket.set_socket_type(SocketType::Bootstrap);
        true
    }

    pub fn to_realtime_connection(&self, node_id: &Account) -> bool {
        if self.socket.socket_type() == SocketType::Undefined && !self.disable_tcp_realtime {
            {
                let mut lk = self.remote_node_id.lock().unwrap();
                *lk = *node_id;
            }

            self.observer.inc_realtime_count();
            self.socket.set_socket_type(SocketType::Realtime);
            return true;
        }
        false
    }

    pub fn set_last_telemetry_req(&self) {
        let mut lk = self.last_telemetry_req.lock().unwrap();
        *lk = Some(Instant::now());
    }

    pub fn unique_id(&self) -> usize {
        self.unique_id
    }

    pub fn is_undefined_connection(&self) -> bool {
        self.socket.socket_type() == SocketType::Undefined
    }

    pub fn is_bootstrap_connection(&self) -> bool {
        self.socket.is_bootstrap_connection()
    }

    pub fn is_realtime_connection(&self) -> bool {
        self.socket.is_realtime_connection()
    }

    pub fn queue_realtime(&self, message: Box<dyn Message>) {
        self.tcp_message_manager.put_message(TcpMessageItem {
            message: Some(message),
            endpoint: *self.remote_endpoint.lock().unwrap(),
            node_id: *self.remote_node_id.lock().unwrap(),
            socket: Some(Arc::clone(&self.socket)),
        });
    }
}

impl Drop for TcpServer {
    fn drop(&mut self) {
        let remote_ep = { *self.remote_endpoint.lock().unwrap() };
        self.observer.boostrap_server_exited(
            self.socket.socket_type(),
            self.unique_id(),
            remote_ep,
        );
        self.stop();
    }
}

pub trait HandshakeMessageVisitor: MessageVisitor {
    fn process(&self) -> bool;
    fn bootstrap(&self) -> bool;
    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor;
}

pub trait RealtimeMessageVisitor: MessageVisitor {
    fn process(&self) -> bool;
    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor;
}

pub trait BootstrapMessageVisitor: MessageVisitor {
    fn processed(&self) -> bool;
    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor;
}

pub trait TcpServerExt {
    fn start(&self);
    fn timeout(&self);

    fn receive_message(&self);
    fn received_message(&self, message: Box<dyn Message>);
    fn process_message(&self, message: Box<dyn Message>) -> bool;
}

impl TcpServerExt for Arc<TcpServer> {
    fn start(&self) {
        // Set remote_endpoint
        let mut guard = self.remote_endpoint.lock().unwrap();
        if guard.port() == 0 {
            if let Some(ep) = self.socket.get_remote() {
                *guard = ep;
            }
            debug_assert!(guard.port() != 0);
        }
        self.receive_message();
    }

    fn timeout(&self) {
        if self.socket.has_timed_out() {
            self.observer.bootstrap_server_timeout(self.unique_id());
            self.socket.close();
        }
    }

    fn receive_message(&self) {
        if self.is_stopped() {
            return;
        }

        let self_clone = Arc::clone(self);
        self.async_rt.tokio.spawn(async move {
            let result = self_clone.message_deserializer.read().await;
            spawn_blocking(Box::new(move || {
                match result {
                    Ok(msg) => self_clone.received_message(msg),
                    Err(ParseStatus::DuplicatePublishMessage) => {
                        self_clone.stats.inc(
                            StatType::Filter,
                            DetailType::DuplicatePublish,
                            Direction::In,
                        );
                        self_clone.receive_message();
                    }
                    Err(ParseStatus::InsufficientWork) => {
                        // IO error or critical error when deserializing message
                        self_clone.stats.inc(
                            StatType::Error,
                            DetailType::InsufficientWork,
                            Direction::In,
                        );
                        self_clone.receive_message();
                    }
                    Err(e) => {
                        // IO error or critical error when deserializing message
                        self_clone
                            .stats
                            .inc(StatType::Error, DetailType::from(e), Direction::In);
                        self_clone.stop();
                    }
                }
            }));
        });
    }

    fn received_message(&self, message: Box<dyn Message>) {
        if self.process_message(message) {
            self.receive_message();
        }
    }

    fn process_message(&self, message: Box<dyn Message>) -> bool {
        let _ = self.stats.inc(
            StatType::TcpServer,
            DetailType::from(message.header().message_type),
            Direction::In,
        );

        debug_assert!(
            self.is_undefined_connection()
                || self.is_realtime_connection()
                || self.is_bootstrap_connection()
        );

        /*
         * Server initially starts in undefined state, where it waits for either a handshake or booststrap request message
         * If the server receives a handshake (and it is successfully validated) it will switch to a realtime mode.
         * In realtime mode messages are deserialized and queued to `tcp_message_manager` for further processing.
         * In realtime mode any bootstrap requests are ignored.
         *
         * If the server receives a bootstrap request before receiving a handshake, it will switch to a bootstrap mode.
         * In bootstrap mode once a valid bootstrap request message is received, the server will start a corresponding bootstrap server and pass control to that server.
         * Once that server finishes its task, control is passed back to this server to read and process any subsequent messages.
         * In bootstrap mode any realtime messages are ignored
         */
        if self.is_undefined_connection() {
            let mut handshake_visitor = self
                .message_visitor_factory
                .handshake_visitor(Arc::clone(self));
            message.visit(handshake_visitor.as_message_visitor());

            if handshake_visitor.process() {
                self.queue_realtime(message);
                return true;
            } else if handshake_visitor.bootstrap() {
                if !self.to_bootstrap_connection() {
                    self.stop();
                    return false;
                }
            } else {
                // Neither handshake nor bootstrap received when in handshake mode
                return true;
            }
        } else if self.is_realtime_connection() {
            let mut realtime_visitor = self
                .message_visitor_factory
                .realtime_visitor(Arc::clone(self));
            message.visit(realtime_visitor.as_message_visitor());
            if realtime_visitor.process() {
                self.queue_realtime(message);
            }
            return true;
        }
        // the server will switch to bootstrap mode immediately after processing the first bootstrap message, thus no `else if`
        if self.is_bootstrap_connection() {
            let mut bootstrap_visitor = self
                .message_visitor_factory
                .bootstrap_visitor(Arc::clone(self));
            message.visit(bootstrap_visitor.as_message_visitor());
            return !bootstrap_visitor.processed(); // Stop receiving new messages if bootstrap serving started
        }
        debug_assert!(false);
        true // Continue receiving new messages
    }
}

pub struct HandshakeMessageVisitorImpl {
    pub process: bool,
    pub bootstrap: bool,
    logger: Arc<dyn Logger>,
    server: Arc<TcpServer>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    node_id: Arc<KeyPair>,
    network_constants: NetworkConstants,
    pub handshake_logging: bool,
    pub disable_tcp_realtime: bool,
}

impl HandshakeMessageVisitorImpl {
    pub fn new(
        server: Arc<TcpServer>,
        logger: Arc<dyn Logger>,
        syn_cookies: Arc<SynCookies>,
        stats: Arc<Stats>,
        node_id: Arc<KeyPair>,
        network_constants: NetworkConstants,
    ) -> Self {
        Self {
            process: false,
            bootstrap: false,
            logger,
            server,
            syn_cookies,
            stats,
            node_id,
            network_constants,
            disable_tcp_realtime: false,
            handshake_logging: false,
        }
    }

    fn prepare_handshake_response(
        &self,
        query: &NodeIdHandshakeQuery,
        v2: bool,
    ) -> NodeIdHandshakeResponse {
        if v2 {
            let genesis = self.server.network.ledger.genesis.hash();
            NodeIdHandshakeResponse::new_v2(&query.cookie, &self.node_id, genesis)
        } else {
            NodeIdHandshakeResponse::new_v1(&query.cookie, &self.node_id)
        }
    }

    fn prepare_handshake_query(
        &self,
        remote_endpoint: &SocketAddr,
    ) -> Option<NodeIdHandshakeQuery> {
        self.syn_cookies
            .assign(remote_endpoint)
            .map(|cookie| NodeIdHandshakeQuery { cookie })
    }

    fn send_handshake_response(&self, query: &NodeIdHandshakeQuery, v2: bool) {
        let response = self.prepare_handshake_response(query, v2);
        let own_query = self.prepare_handshake_query(&self.server.remote_endpoint());
        let handshake_response = NodeIdHandshake::new(
            &self.network_constants.protocol_info(),
            own_query,
            Some(response),
        );

        let mut stream = MemoryStream::new();
        handshake_response.serialize(&mut stream).unwrap();

        let shared_const_buffer = Arc::new(stream.to_vec());
        let server_weak = Arc::downgrade(&self.server);
        let logger = Arc::clone(&self.logger);
        let stats = Arc::clone(&self.stats);
        let handshake_logging = self.handshake_logging;
        self.server.socket.async_write(
            &shared_const_buffer,
            Some(Box::new(move |ec, _size| {
                if let Some(server_l) = server_weak.upgrade() {
                    if ec.is_err() {
                        if handshake_logging {
                            logger.try_log(&format!(
                                "Error sending node_id_handshake to {}: {:?}",
                                server_l.remote_endpoint(),
                                ec
                            ));
                        }
                        // Stop invalid handshake
                        server_l.stop();
                    } else {
                        let _ = stats.inc(
                            StatType::Message,
                            DetailType::NodeIdHandshake,
                            Direction::Out,
                        );
                    }
                }
            })),
            super::TrafficType::Generic,
        );
    }

    fn verify_handshake_response(
        &self,
        response: &NodeIdHandshakeResponse,
        remote_endpoint: &SocketAddr,
    ) -> bool {
        // Prevent connection with ourselves
        if response.node_id == self.node_id.public_key() {
            self.stats.inc(
                StatType::Handshake,
                DetailType::InvalidNodeId,
                Direction::In,
            );
            return false; // Fail
        }

        // Prevent mismatched genesis
        if let Some(v2) = &response.v2 {
            if v2.genesis != self.server.network.ledger.genesis.hash() {
                self.stats.inc(
                    StatType::Handshake,
                    DetailType::InvalidGenesis,
                    Direction::In,
                );
                return false; // Fail
            }
        }

        let Some(cookie) = self.syn_cookies.cookie(remote_endpoint) else {
            self.stats.inc(
                StatType::Handshake,
                DetailType::MissingCookie,
                Direction::In,
            );
            return false; // Fail
        };

        if response.validate(&cookie).is_err() {
            self.stats.inc(
                StatType::Handshake,
                DetailType::InvalidSignature,
                Direction::In,
            );
            return false; // Fail
        }

        self.stats
            .inc(StatType::Handshake, DetailType::Ok, Direction::In);
        true // OK
    }
}

impl MessageVisitor for HandshakeMessageVisitorImpl {
    fn keepalive(&mut self, message: &MessageEnum) {
        self.bootstrap = matches!(
            &message.payload,
            Payload::BulkPull(_)
                | Payload::BulkPullAccount(_)
                | Payload::BulkPush
                | Payload::FrontierReq(_)
        );
    }

    fn node_id_handshake(&mut self, message: &NodeIdHandshake) {
        if self.disable_tcp_realtime {
            if self.handshake_logging {
                self.logger.try_log(&format!(
                    "Disabled realtime TCP for handshake {}",
                    self.server.remote_endpoint()
                ));
            }
            // Stop invalid handshake
            self.server.stop();
            return;
        }

        if message.query.is_some() && self.server.was_handshake_query_received() {
            if self.handshake_logging {
                self.logger.try_log(&format!(
                    "Detected multiple node_id_handshake query from {}",
                    self.server.remote_endpoint()
                ));
            }
            // Stop invalid handshake
            self.server.stop();
            return;
        }

        self.server.handshake_query_received();

        if self.handshake_logging {
            self.logger.try_log(&format!(
                "Received node_id_handshake message from {}",
                self.server.remote_endpoint()
            ));
        }

        if let Some(query) = &message.query {
            self.send_handshake_response(query, message.is_v2());
        } else if let Some(response) = &message.response {
            if self.verify_handshake_response(response, &self.server.remote_endpoint()) {
                self.server.to_realtime_connection(&response.node_id);
            } else {
                // Stop invalid handshake
                self.server.stop();
                return;
            }
        }

        self.process = true;
    }
}

impl HandshakeMessageVisitor for HandshakeMessageVisitorImpl {
    fn process(&self) -> bool {
        self.process
    }

    fn bootstrap(&self) -> bool {
        self.bootstrap
    }

    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor {
        self
    }
}

pub struct RealtimeMessageVisitorImpl {
    server: Arc<TcpServer>,
    stats: Arc<Stats>,
    process: bool,
}

impl RealtimeMessageVisitorImpl {
    pub fn new(server: Arc<TcpServer>, stats: Arc<Stats>) -> Self {
        Self {
            server,
            stats,
            process: false,
        }
    }
}

impl MessageVisitor for RealtimeMessageVisitorImpl {
    fn keepalive(&mut self, message: &MessageEnum) {
        match &message.payload {
            Payload::Keepalive(_)
            | Payload::Publish(_)
            | Payload::AscPullAck(_)
            | Payload::AscPullReq(_)
            | Payload::ConfirmAck(_)
            | Payload::ConfirmReq(_)
            | Payload::FrontierReq(_) => self.process = true,
            _ => {}
        }
    }
    fn telemetry_req(&mut self, _message: &TelemetryReq) {
        // Only handle telemetry requests if they are outside of the cooldown period
        if self.server.is_outside_cooldown_period() {
            self.server.set_last_telemetry_req();
            self.process = true;
        } else {
            let _ = self.stats.inc(
                StatType::Telemetry,
                DetailType::RequestWithinProtectionCacheZone,
                Direction::In,
            );
        }
    }
    fn telemetry_ack(&mut self, _message: &TelemetryAck) {
        self.process = true;
    }
}

impl RealtimeMessageVisitor for RealtimeMessageVisitorImpl {
    fn process(&self) -> bool {
        self.process
    }

    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor {
        self
    }
}
