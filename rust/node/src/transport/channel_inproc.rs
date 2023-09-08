use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use message_deserializer::MessageDeserializer;
use rsnano_core::Account;

use crate::{
    config::NetworkConstants,
    messages::Message,
    stats::{DetailType, Direction, StatType, Stats},
    transport::message_deserializer,
    utils::{AsyncRuntime, BlockUniquer, ErrorCode},
    voting::VoteUniquer,
};

use super::{
    message_deserializer::ReadQuery, BandwidthLimitType, BufferDropPolicy, Channel, ChannelEnum,
    MessageDeserializerExt, NetworkFilter, OutboundBandwidthLimiter, TrafficType, WriteCallback,
};

pub struct InProcChannelData {
    last_bootstrap_attempt: SystemTime,
    last_packet_received: SystemTime,
    last_packet_sent: SystemTime,
    node_id: Option<Account>,
}

pub type InboundCallback = Arc<dyn Fn(Box<dyn Message>, Arc<ChannelEnum>) + Send + Sync>;

pub struct ChannelInProc {
    channel_id: usize,
    temporary: AtomicBool,
    channel_mutex: Mutex<InProcChannelData>,
    network_constants: NetworkConstants,
    network_filter: Arc<NetworkFilter>,
    stats: Arc<Stats>,
    limiter: Arc<OutboundBandwidthLimiter>,
    source_inbound: InboundCallback,
    destination_inbound: InboundCallback,
    async_rt: Weak<AsyncRuntime>,
    pub source_endpoint: SocketAddr,
    pub destination_endpoint: SocketAddr,
    source_node_id: Account,
    destination_node_id: Account,
}

impl ChannelInProc {
    pub fn new(
        channel_id: usize,
        now: SystemTime,
        network_constants: NetworkConstants,
        network_filter: Arc<NetworkFilter>,
        stats: Arc<Stats>,
        limiter: Arc<OutboundBandwidthLimiter>,
        source_inbound: InboundCallback,
        destination_inbound: InboundCallback,
        async_rt: &Arc<AsyncRuntime>,
        source_endpoint: SocketAddr,
        destination_endpoint: SocketAddr,
        source_node_id: Account,
        destination_node_id: Account,
    ) -> Self {
        Self {
            channel_id,
            temporary: AtomicBool::new(false),
            channel_mutex: Mutex::new(InProcChannelData {
                last_bootstrap_attempt: UNIX_EPOCH,
                last_packet_received: now,
                last_packet_sent: now,
                node_id: Some(source_node_id),
            }),
            network_constants,
            network_filter,
            stats,
            limiter,
            source_inbound,
            destination_inbound,
            async_rt: Arc::downgrade(async_rt),
            source_endpoint,
            destination_endpoint,
            source_node_id,
            destination_node_id,
        }
    }

    pub fn send(
        &self,
        message_a: &dyn Message,
        callback_a: Option<WriteCallback>,
        drop_policy: BufferDropPolicy,
        traffic_type: TrafficType,
    ) {
        let buffer = Arc::new(message_a.to_bytes());
        let detail = DetailType::from(message_a.header().message_type());
        let is_droppable_by_limiter = drop_policy == BufferDropPolicy::Limiter;
        let should_pass = self
            .limiter
            .should_pass(buffer.len(), BandwidthLimitType::from(traffic_type));

        if !is_droppable_by_limiter || should_pass {
            self.send_buffer_2(&buffer, callback_a, drop_policy, traffic_type);
            self.stats.inc(StatType::Message, detail, Direction::Out);
        } else {
            if let Some(cb) = callback_a {
                if let Some(async_rt) = self.async_rt.upgrade() {
                    async_rt.post(Box::new(move || {
                        cb(ErrorCode::not_supported(), 0);
                    }))
                }
            }

            self.stats.inc(StatType::Drop, detail, Direction::Out);
        }
    }

    pub fn send_buffer_2(
        &self,
        buffer_a: &Arc<Vec<u8>>,
        callback_a: Option<WriteCallback>,
        _policy_a: BufferDropPolicy,
        _traffic_type: TrafficType,
    ) {
        let stats = self.stats.clone();
        let network_constants = self.network_constants.clone();
        let limiter = self.limiter.clone();
        let source_inbound = self.source_inbound.clone();
        let destination_inbound = self.destination_inbound.clone();
        let source_endpoint = self.source_endpoint;
        let destination_endpoint = self.destination_endpoint;
        let source_node_id = self.source_node_id;
        let destination_node_id = self.destination_node_id;
        let async_rt = self.async_rt.clone();

        let callback_wrapper = Box::new(move |ec: ErrorCode, msg: Option<Box<dyn Message>>| {
            if ec.is_err() {
                return;
            }
            let Some(async_rt) = async_rt.upgrade() else { return; };
            let Some(msg) = msg else { return; };
            let filter = Arc::new(NetworkFilter::new(100000));
            // we create a temporary channel for the reply path, in case the receiver of the message wants to reply
            let remote_channel = Arc::new(ChannelEnum::InProc(ChannelInProc::new(
                1,
                SystemTime::now(),
                network_constants.clone(),
                filter,
                stats.clone(),
                limiter,
                source_inbound,
                destination_inbound.clone(),
                &async_rt,
                source_endpoint,
                destination_endpoint,
                source_node_id,
                destination_node_id,
            )));

            // process message
            {
                stats.inc(
                    StatType::Message,
                    DetailType::from(msg.header().message_type()),
                    Direction::In,
                );

                destination_inbound(msg, remote_channel);
            }
        });

        self.send_buffer_impl(buffer_a, callback_wrapper);

        if let Some(cb) = callback_a {
            let buffer_size = buffer_a.len();
            if let Some(async_rt) = self.async_rt.upgrade() {
                async_rt.post(Box::new(move || {
                    cb(ErrorCode::new(), buffer_size);
                }));
            }
        }
    }

    fn send_buffer_impl(
        &self,
        buffer: &[u8],
        callback_msg: Box<dyn FnOnce(ErrorCode, Option<Box<dyn Message>>) + Send>,
    ) {
        let offset = AtomicUsize::new(0);
        let buffer_copy = buffer.to_vec();
        let buffer_read_fn: ReadQuery = Box::new(move |data, size, callback| {
            let os = offset.load(Ordering::SeqCst);
            debug_assert!(buffer_copy.len() >= (os + size));
            let mut data_lock = data.lock().unwrap();
            data_lock.resize(size, 0);
            data_lock.copy_from_slice(&buffer_copy[os..(os + size)]);
            drop(data_lock);
            offset.fetch_add(size, Ordering::SeqCst);
            callback(ErrorCode::new(), size);
        });

        let message_deserializer = Arc::new(MessageDeserializer::new(
            self.network_constants.clone(),
            self.network_filter.clone(),
            Arc::new(BlockUniquer::new()),
            Arc::new(VoteUniquer::new()),
            buffer_read_fn,
        ));
        message_deserializer.read(callback_msg);
    }

    pub fn network_version(&self) -> u8 {
        self.network_constants.protocol_version
    }
}

impl Channel for ChannelInProc {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary
            .store(temporary, std::sync::atomic::Ordering::SeqCst);
    }

    fn get_last_bootstrap_attempt(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt
    }

    fn set_last_bootstrap_attempt(&self, time: SystemTime) {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt = time;
    }

    fn get_last_packet_received(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_received
    }

    fn set_last_packet_received(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_received = instant;
    }

    fn get_last_packet_sent(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_sent
    }

    fn set_last_packet_sent(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_sent = instant;
    }

    fn get_node_id(&self) -> Option<Account> {
        self.channel_mutex.lock().unwrap().node_id
    }

    fn set_node_id(&self, id: Account) {
        self.channel_mutex.lock().unwrap().node_id = Some(id);
    }

    fn is_alive(&self) -> bool {
        true
    }

    fn channel_id(&self) -> usize {
        self.channel_id
    }

    fn get_type(&self) -> super::TransportType {
        super::TransportType::Loopback
    }
}
