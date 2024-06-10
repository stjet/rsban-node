use super::{ChannelEnum, HandshakeProcess, MessageDeserializer, Network, NetworkFilter};
use crate::{
    bootstrap::BootstrapMessageVisitorFactory,
    config::NodeConfig,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{
        NetworkExt, Socket, SocketExtensions, SocketType, SynCookies, TcpMessageManager,
        TrafficType,
    },
    utils::AsyncRuntime,
    NetworkParams,
};
use rsnano_core::{utils::NULL_ENDPOINT, Account, KeyPair};
use rsnano_messages::*;
use std::{
    net::SocketAddrV6,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::{sync::Notify, task::spawn_blocking};
use tracing::debug;

#[derive(Clone, Debug)]
pub struct TcpConfig {
    pub max_inbound_connections: usize,
    pub max_outbound_connections: usize,
    pub max_attempts: usize,
    pub max_attempts_per_ip: usize,
    pub connect_timeout: Duration,
}

impl TcpConfig {
    pub fn for_dev_network() -> Self {
        Self {
            max_inbound_connections: 128,
            max_outbound_connections: 128,
            max_attempts: 128,
            max_attempts_per_ip: 128,
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            max_inbound_connections: 2048,
            max_outbound_connections: 2048,
            max_attempts: 60,
            max_attempts_per_ip: 1,
            connect_timeout: Duration::from_secs(60),
        }
    }
}

pub trait ResponseServerObserver: Send + Sync {
    fn bootstrap_server_timeout(&self, connection_id: usize);
    fn boostrap_server_exited(
        &self,
        socket_type: SocketType,
        connection_id: usize,
        endpoint: SocketAddrV6,
    );
    fn bootstrap_count(&self) -> usize;
    fn inc_bootstrap_count(&self);
    fn inc_realtime_count(&self);
    fn dec_bootstrap_count(&self);
    fn dec_realtime_count(&self);
}

pub struct NullTcpServerObserver {}
impl ResponseServerObserver for NullTcpServerObserver {
    fn bootstrap_server_timeout(&self, _inner_ptr: usize) {}

    fn boostrap_server_exited(
        &self,
        _socket_type: SocketType,
        _unique_id: usize,
        _endpoint: SocketAddrV6,
    ) {
    }

    fn bootstrap_count(&self) -> usize {
        0
    }

    fn inc_bootstrap_count(&self) {}
    fn inc_realtime_count(&self) {}
    fn dec_realtime_count(&self) {}
    fn dec_bootstrap_count(&self) {}
}

pub trait ResponseServer {}

pub struct ResponseServerImpl {
    async_rt: Weak<AsyncRuntime>,
    channel: Mutex<Option<Arc<ChannelEnum>>>,
    pub socket: Arc<Socket>,
    config: Arc<NodeConfig>,
    stopped: AtomicBool,
    observer: Weak<dyn ResponseServerObserver>,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,

    // Remote enpoint used to remove response channel even after socket closing
    remote_endpoint: Mutex<SocketAddrV6>,

    network_params: Arc<NetworkParams>,
    last_telemetry_req: Mutex<Option<Instant>>,
    unique_id: usize,
    stats: Arc<Stats>,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    message_visitor_factory: Arc<BootstrapMessageVisitorFactory>,
    message_deserializer: Arc<MessageDeserializer<Arc<Socket>>>,
    tcp_message_manager: Arc<TcpMessageManager>,
    allow_bootstrap: bool,
    notify_stop: Notify,
    last_keepalive: Mutex<Option<Keepalive>>,
    syn_cookies: Arc<SynCookies>,
    node_id: KeyPair,
    protocol_info: ProtocolInfo,
    network: Weak<Network>,
    handshake_process: HandshakeProcess,
}

static NEXT_UNIQUE_ID: AtomicUsize = AtomicUsize::new(0);

impl ResponseServerImpl {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        network: &Arc<Network>,
        socket: Arc<Socket>,
        config: Arc<NodeConfig>,
        observer: Weak<dyn ResponseServerObserver>,
        publish_filter: Arc<NetworkFilter>,
        network_params: Arc<NetworkParams>,
        stats: Arc<Stats>,
        tcp_message_manager: Arc<TcpMessageManager>,
        message_visitor_factory: Arc<BootstrapMessageVisitorFactory>,
        allow_bootstrap: bool,
        syn_cookies: Arc<SynCookies>,
        node_id: KeyPair,
    ) -> Self {
        let network_constants = network_params.network.clone();
        let socket_clone = Arc::clone(&socket);
        debug!(
            socket_id = socket.socket_id,
            "Cloning socket in TcpServer constructor"
        );
        let remote_endpoint = socket.get_remote().unwrap_or(NULL_ENDPOINT);
        Self {
            async_rt: Arc::downgrade(&async_rt),
            network: Arc::downgrade(network),
            socket,
            channel: Mutex::new(None),
            config,
            observer,
            stopped: AtomicBool::new(false),
            disable_bootstrap_listener: false,
            connections_max: 64,
            remote_endpoint: Mutex::new(remote_endpoint),
            last_telemetry_req: Mutex::new(None),
            handshake_process: HandshakeProcess::new(
                network_params.ledger.genesis.hash(),
                node_id.clone(),
                syn_cookies.clone(),
                stats.clone(),
                remote_endpoint,
            ),
            network_params,
            unique_id: NEXT_UNIQUE_ID.fetch_add(1, Ordering::Relaxed),
            stats,
            disable_bootstrap_bulk_pull_server: false,
            disable_tcp_realtime: false,
            message_visitor_factory,
            protocol_info: network_constants.protocol_info(),
            message_deserializer: Arc::new(MessageDeserializer::new(
                network_constants.protocol_info(),
                network_constants.work.clone(),
                publish_filter,
                socket_clone,
            )),
            tcp_message_manager,
            allow_bootstrap,
            notify_stop: Notify::new(),
            last_keepalive: Mutex::new(None),
            syn_cookies,
            node_id,
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.socket.close();
            self.notify_stop.notify_one();
        }
    }

    pub fn remote_endpoint(&self) -> SocketAddrV6 {
        *self.remote_endpoint.lock().unwrap()
    }

    fn is_outside_cooldown_period(&self) -> bool {
        let lock = self.last_telemetry_req.lock().unwrap();
        match *lock {
            Some(last_req) => {
                last_req.elapsed()
                    >= Duration::from_millis(
                        self.network_params.network.telemetry_request_cooldown_ms as u64,
                    )
            }
            None => true,
        }
    }

    fn to_bootstrap_connection(&self) -> bool {
        if !self.allow_bootstrap {
            return false;
        }

        if self.disable_bootstrap_listener {
            return false;
        }

        let Some(observer) = self.observer.upgrade() else {
            return false;
        };

        if observer.bootstrap_count() >= self.connections_max {
            return false;
        }

        if self.socket.socket_type() != SocketType::Undefined {
            return false;
        }

        observer.inc_bootstrap_count();
        self.socket.set_socket_type(SocketType::Bootstrap);
        debug!("Switched to bootstrap mode ({})", self.remote_endpoint());
        true
    }

    fn set_last_telemetry_req(&self) {
        let mut lk = self.last_telemetry_req.lock().unwrap();
        *lk = Some(Instant::now());
    }

    pub fn unique_id(&self) -> usize {
        self.unique_id
    }

    fn is_undefined_connection(&self) -> bool {
        self.socket.socket_type() == SocketType::Undefined
    }

    fn is_bootstrap_connection(&self) -> bool {
        self.socket.is_bootstrap_connection()
    }

    fn is_realtime_connection(&self) -> bool {
        self.socket.is_realtime_connection()
    }

    fn queue_realtime(&self, message: DeserializedMessage) {
        let channel = self.channel.lock().unwrap().as_ref().unwrap().clone();
        channel.set_last_packet_received(SystemTime::now());
        self.tcp_message_manager.put(message, channel);
    }

    fn set_last_keepalive(&self, keepalive: Option<Keepalive>) {
        *self.last_keepalive.lock().unwrap() = keepalive;
    }

    pub fn get_last_keepalive(&self) -> Option<Keepalive> {
        self.last_keepalive.lock().unwrap().clone()
    }

    pub fn pop_last_keepalive(&self) -> Option<Keepalive> {
        self.last_keepalive.lock().unwrap().take()
    }
}

impl Drop for ResponseServerImpl {
    fn drop(&mut self) {
        let remote_ep = { *self.remote_endpoint.lock().unwrap() };
        if let Some(observer) = self.observer.upgrade() {
            observer.boostrap_server_exited(self.socket.socket_type(), self.unique_id(), remote_ep);
        }
        self.stop();
    }
}

pub trait RealtimeMessageVisitor: MessageVisitor {
    fn process(&self) -> bool;
    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor;
}

pub trait BootstrapMessageVisitor: MessageVisitor {
    fn processed(&self) -> bool;
    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor;
}

pub trait ResponseServerExt {
    fn start(&self);
    fn timeout(&self);

    fn to_realtime_connection(&self, node_id: &Account) -> bool;
    fn initiate_handshake(&self);
    fn receive_message(&self);
    fn received_message(&self, message: DeserializedMessage);
    fn process_message(&self, message: DeserializedMessage) -> ProcessResult;
    fn process_handshake(&self, message: &NodeIdHandshake) -> HandshakeStatus;
    fn send_handshake_response(&self, query: &NodeIdHandshakeQuery, v2: bool);
}

pub enum HandshakeStatus {
    Abort,
    Handshake,
    Realtime,
    Bootstrap,
}

pub enum ProcessResult {
    Abort,
    Progress,
    Pause,
}

impl ResponseServerExt for Arc<ResponseServerImpl> {
    fn start(&self) {
        // Set remote_endpoint
        let mut guard = self.remote_endpoint.lock().unwrap();
        if guard.port() == 0 {
            if let Some(ep) = self.socket.get_remote() {
                *guard = ep;
            }
            //debug_assert!(guard.port() != 0);
        }

        debug!("Starting server: {}", guard.port());
        self.receive_message();
    }

    fn to_realtime_connection(&self, node_id: &Account) -> bool {
        let Some(observer) = self.observer.upgrade() else {
            return false;
        };

        if self.disable_tcp_realtime {
            return false;
        }

        if self.socket.socket_type() != SocketType::Undefined {
            return false;
        }

        let Some(network) = self.network.upgrade() else {
            return false;
        };

        let Some(channel) = network.create(Arc::clone(&self.socket), Arc::clone(self), *node_id)
        else {
            return false;
        };

        *self.channel.lock().unwrap() = Some(channel);
        observer.inc_realtime_count();
        self.socket.set_socket_type(SocketType::Realtime);
        debug!("Switched to realtime mode ({})", self.remote_endpoint());
        return true;
    }

    fn timeout(&self) {
        if self.socket.has_timed_out() {
            if let Some(observer) = self.observer.upgrade() {
                observer.bootstrap_server_timeout(self.unique_id());
            }
            self.socket.close();
        }
    }

    fn initiate_handshake(&self) {
        let endpoint = self.remote_endpoint();
        let query = self.handshake_process.prepare_query(&endpoint);
        let message = Message::NodeIdHandshake(NodeIdHandshake {
            query,
            response: None,
            is_v2: true,
        });

        debug!("Initiating handshake query ({})", endpoint);

        let mut serializer = MessageSerializer::new(self.network_params.network.protocol_info());
        let buffer = Arc::new(serializer.serialize(&message).to_vec());
        let self_w = Arc::downgrade(self);

        self.socket.async_write(
            &buffer,
            Some(Box::new(move |ec, _len| {
                if let Some(self_l) = self_w.upgrade() {
                    if ec.is_ok() {
                        self_l.stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::Handshake,
                            Direction::Out,
                        );
                        self_l.stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::HandshakeInitiate,
                            Direction::Out,
                        );
                    } else {
                        self_l
                            .stats
                            .inc(StatType::TcpServer, DetailType::HandshakeNetworkError);
                        debug!("Error sending handshake query: {:?} ({})", ec, endpoint);

                        // Stop invalid handshake
                        self_l.stop();
                    }
                }
            })),
            TrafficType::Generic,
        );
    }

    fn receive_message(&self) {
        if self.is_stopped() {
            return;
        }

        let Some(async_rt) = self.async_rt.upgrade() else {
            return;
        };

        let self_clone = Arc::clone(self);
        async_rt.tokio.spawn(async move {
            let result = tokio::select! {
                i = self_clone.message_deserializer.read() => i,
                _ = self_clone.notify_stop.notified() => Err(ParseMessageError::Other)
            };

            spawn_blocking(Box::new(move || {
                match result {
                    Ok(msg) => self_clone.received_message(msg),
                    Err(ParseMessageError::DuplicatePublishMessage) => {
                        // Avoid too much noise about `duplicate_publish_message` errors
                        self_clone.stats.inc_dir(
                            StatType::Filter,
                            DetailType::DuplicatePublishMessage,
                            Direction::In,
                        );
                        self_clone.receive_message();
                    }
                    Err(ParseMessageError::InsufficientWork) => {
                        // IO error or critical error when deserializing message
                        self_clone.stats.inc_dir(
                            StatType::Error,
                            DetailType::InsufficientWork,
                            Direction::In,
                        );
                        self_clone.receive_message();
                    }
                    Err(e) => {
                        // IO error or critical error when deserializing message
                        self_clone.stats.inc_dir(
                            StatType::Error,
                            DetailType::from(e),
                            Direction::In,
                        );
                        debug!(
                            "Error reading message: {:?} ({})",
                            e,
                            self_clone.remote_endpoint()
                        );

                        self_clone.stop();
                    }
                }
            }));
        });
    }

    fn received_message(&self, message: DeserializedMessage) {
        match self.process_message(message) {
            ProcessResult::Progress => self.receive_message(),
            ProcessResult::Abort => self.stop(),
            ProcessResult::Pause => {
                // Do nothing
            }
        }
    }

    fn process_message(&self, message: DeserializedMessage) -> ProcessResult {
        self.stats.inc_dir(
            StatType::TcpServer,
            DetailType::from(message.message.message_type()),
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
            let mut handshake_visitor = HandshakeMessageVisitor::new(Arc::clone(&self));
            handshake_visitor.received(&message.message);

            match handshake_visitor.result {
                HandshakeStatus::Abort => {
                    self.stats.inc_dir(
                        StatType::TcpServer,
                        DetailType::HandshakeAbort,
                        Direction::In,
                    );
                    debug!(
                        "Aborting handshake: {:?} ({})",
                        message.message.message_type(),
                        self.remote_endpoint()
                    );
                    return ProcessResult::Abort;
                }
                HandshakeStatus::Handshake => {
                    return ProcessResult::Progress; // Continue handshake
                }
                HandshakeStatus::Realtime => {
                    self.queue_realtime(message);
                    return ProcessResult::Progress; // Continue receiving new messages
                }
                HandshakeStatus::Bootstrap => {
                    if !self.to_bootstrap_connection() {
                        self.stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::HandshakeError,
                            Direction::In,
                        );
                        debug!(
                            "Error switching to bootstrap mode: {:?} ({})",
                            message.message.message_type(),
                            self.remote_endpoint()
                        );
                        return ProcessResult::Abort;
                    } else {
                        // Fall through to process the bootstrap message
                    }
                }
            }
        } else if self.is_realtime_connection() {
            let mut realtime_visitor = self
                .message_visitor_factory
                .realtime_visitor(Arc::clone(self));
            realtime_visitor.received(&message.message);
            if realtime_visitor.process() {
                self.queue_realtime(message);
            }
            return ProcessResult::Progress;
        }
        // The server will switch to bootstrap mode immediately after processing the first bootstrap message, thus no `else if`
        if self.is_bootstrap_connection() {
            let mut bootstrap_visitor = self
                .message_visitor_factory
                .bootstrap_visitor(Arc::clone(self));
            bootstrap_visitor.received(&message.message);

            // Pause receiving new messages if bootstrap serving started
            return if bootstrap_visitor.processed() {
                ProcessResult::Pause
            } else {
                ProcessResult::Progress
            };
        }
        debug_assert!(false);
        ProcessResult::Abort
    }

    fn process_handshake(&self, message: &NodeIdHandshake) -> HandshakeStatus {
        if self.disable_tcp_realtime {
            self.stats.inc_dir(
                StatType::TcpServer,
                DetailType::HandshakeError,
                Direction::In,
            );
            debug!(
                "Handshake attempted with disabled realtime mode ({})",
                self.remote_endpoint()
            );
            return HandshakeStatus::Abort;
        }
        if message.query.is_none() && message.response.is_none() {
            self.stats.inc_dir(
                StatType::TcpServer,
                DetailType::HandshakeError,
                Direction::In,
            );
            debug!(
                "Invalid handshake message received ({})",
                self.remote_endpoint()
            );
            return HandshakeStatus::Abort;
        }
        if message.query.is_some()
            && self
                .handshake_process
                .handshake_received
                .load(Ordering::SeqCst)
        {
            // Second handshake message should be a response only
            self.stats.inc_dir(
                StatType::TcpServer,
                DetailType::HandshakeError,
                Direction::In,
            );
            debug!(
                "Detected multiple handshake queries ({})",
                self.remote_endpoint()
            );
            return HandshakeStatus::Abort;
        }

        self.handshake_process
            .handshake_received
            .store(true, Ordering::SeqCst);

        self.stats.inc_dir(
            StatType::TcpServer,
            DetailType::NodeIdHandshake,
            Direction::In,
        );

        let log_type = match (message.query.is_some(), message.response.is_some()) {
            (true, true) => "query + response",
            (true, false) => "query",
            (false, true) => "response",
            (false, false) => "none",
        };
        debug!(
            "Handshake message received: {} ({})",
            log_type,
            self.remote_endpoint()
        );

        if let Some(query) = &message.query {
            // Send response + our own query
            self.send_handshake_response(query, message.is_v2);
            // Fall through and continue handshake
        }
        if let Some(response) = &message.response {
            if self
                .handshake_process
                .verify_response(response, &self.remote_endpoint())
            {
                let success = self.to_realtime_connection(&response.node_id);
                if success {
                    return HandshakeStatus::Realtime; // Switch to realtime
                } else {
                    self.stats.inc_dir(
                        StatType::TcpServer,
                        DetailType::HandshakeError,
                        Direction::In,
                    );
                    debug!(
                        "Error switching to realtime mode ({})",
                        self.remote_endpoint()
                    );
                    return HandshakeStatus::Abort;
                }
            } else {
                self.stats.inc_dir(
                    StatType::TcpServer,
                    DetailType::HandshakeResponseInvalid,
                    Direction::In,
                );
                debug!(
                    "Invalid handshake response received ({})",
                    self.remote_endpoint()
                );
                return HandshakeStatus::Abort;
            }
        }
        HandshakeStatus::Handshake // Handshake is in progress
    }

    fn send_handshake_response(&self, query: &NodeIdHandshakeQuery, v2: bool) {
        let response = self.handshake_process.prepare_response(query, v2);
        let own_query = self
            .handshake_process
            .prepare_query(&self.remote_endpoint());
        let handshake_response = Message::NodeIdHandshake(NodeIdHandshake {
            is_v2: own_query.is_some() || response.v2.is_some(),
            query: own_query,
            response: Some(response),
        });

        debug!("Responding to handshake ({})", self.remote_endpoint());

        let mut serializer = MessageSerializer::new(self.protocol_info);
        let buffer = serializer.serialize(&handshake_response);
        let shared_const_buffer = Arc::new(Vec::from(buffer)); // TODO don't copy buffer
        let server_weak = Arc::downgrade(&self);
        let stats = Arc::clone(&self.stats);
        self.socket.async_write(
            &shared_const_buffer,
            Some(Box::new(move |ec, _size| {
                if let Some(server_l) = server_weak.upgrade() {
                    if ec.is_err() {
                        server_l.stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::HandshakeNetworkError,
                            Direction::In,
                        );
                        debug!(
                            "Error sending handshake response: {} ({:?})",
                            server_l.remote_endpoint(),
                            ec
                        );
                        // Stop invalid handshake
                        server_l.stop();
                    } else {
                        let _ = stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::Handshake,
                            Direction::Out,
                        );
                        stats.inc_dir(
                            StatType::TcpServer,
                            DetailType::HandshakeResponse,
                            Direction::Out,
                        );
                    }
                }
            })),
            super::TrafficType::Generic,
        );
    }
}

struct HandshakeMessageVisitor {
    pub result: HandshakeStatus,
    server: Arc<ResponseServerImpl>,
}

impl HandshakeMessageVisitor {
    fn new(server: Arc<ResponseServerImpl>) -> Self {
        Self {
            server,
            result: HandshakeStatus::Abort,
        }
    }
}

impl MessageVisitor for HandshakeMessageVisitor {
    fn received(&mut self, message: &Message) {
        self.result = match message {
            Message::BulkPull(_)
            | Message::BulkPullAccount(_)
            | Message::BulkPush
            | Message::FrontierReq(_) => HandshakeStatus::Bootstrap,
            Message::NodeIdHandshake(payload) => self.server.process_handshake(payload),
            _ => HandshakeStatus::Abort,
        }
    }
}

pub struct RealtimeMessageVisitorImpl {
    server: Arc<ResponseServerImpl>,
    stats: Arc<Stats>,
    process: bool,
}

impl RealtimeMessageVisitorImpl {
    pub fn new(server: Arc<ResponseServerImpl>, stats: Arc<Stats>) -> Self {
        Self {
            server,
            stats,
            process: false,
        }
    }
}

impl MessageVisitor for RealtimeMessageVisitorImpl {
    fn received(&mut self, message: &Message) {
        match message {
            Message::Keepalive(keepalive) => {
                self.process = true;
                self.server.set_last_keepalive(Some(keepalive.clone()));
            }
            Message::Publish(_)
            | Message::AscPullAck(_)
            | Message::AscPullReq(_)
            | Message::ConfirmAck(_)
            | Message::ConfirmReq(_)
            | Message::FrontierReq(_)
            | Message::TelemetryAck(_) => self.process = true,
            Message::TelemetryReq => {
                // Only handle telemetry requests if they are outside of the cooldown period
                if self.server.is_outside_cooldown_period() {
                    self.server.set_last_telemetry_req();
                    self.process = true;
                } else {
                    self.stats.inc_dir(
                        StatType::Telemetry,
                        DetailType::RequestWithinProtectionCacheZone,
                        Direction::In,
                    );
                }
            }
            _ => {}
        }
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
