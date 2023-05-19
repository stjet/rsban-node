use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use rsnano_core::Account;

use crate::{
    messages::Message,
    stats::{DetailType, Direction, StatType, Stats},
    utils::{ErrorCode, IoContext},
};

use super::{
    BandwidthLimitType, BufferDropPolicy, Channel, OutboundBandwidthLimiter, TrafficType,
    WriteCallback,
};

pub struct FakeChannelData {
    last_bootstrap_attempt: u64,
    last_packet_received: u64,
    last_packet_sent: u64,
    node_id: Option<Account>,
}

pub struct ChannelFake {
    channel_id: usize,
    io_ctx: Box<dyn IoContext>,
    temporary: AtomicBool,
    channel_mutex: Mutex<FakeChannelData>,
    limiter: Arc<OutboundBandwidthLimiter>,
    stats: Arc<Stats>,
    endpoint: SocketAddr,
    closed: AtomicBool,
    network_version: u8,
}

impl ChannelFake {
    pub fn new(
        now: u64,
        channel_id: usize,
        io_ctx: Box<dyn IoContext>,
        limiter: Arc<OutboundBandwidthLimiter>,
        stats: Arc<Stats>,
        endpoint: SocketAddr,
        network_version: u8,
    ) -> Self {
        Self {
            channel_id,
            io_ctx,
            temporary: AtomicBool::new(false),
            channel_mutex: Mutex::new(FakeChannelData {
                last_bootstrap_attempt: 0,
                last_packet_received: now,
                last_packet_sent: now,
                node_id: None,
            }),
            limiter,
            stats,
            endpoint,
            closed: AtomicBool::new(false),
            network_version,
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
            self.send_buffer(&buffer, callback_a, drop_policy, traffic_type);
            self.stats.inc(StatType::Message, detail, Direction::Out);
        } else {
            if let Some(cb) = callback_a {
                self.io_ctx.post(Box::new(move || {
                    cb(ErrorCode::not_supported(), 0);
                }))
            }

            self.stats.inc(StatType::Drop, detail, Direction::Out);
        }
    }

    pub fn send_buffer(
        &self,
        buffer_a: &Arc<Vec<u8>>,
        callback_a: Option<WriteCallback>,
        _policy_a: BufferDropPolicy,
        _traffic_type: TrafficType,
    ) {
        let size = buffer_a.len();
        if let Some(cb) = callback_a {
            self.io_ctx.post(Box::new(move || {
                cb(ErrorCode::new(), size);
            }))
        }
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn endpoint(&self) -> &SocketAddr {
        &self.endpoint
    }

    pub fn network_version(&self) -> u8 {
        self.network_version
    }
}

impl Channel for ChannelFake {
    fn is_temporary(&self) -> bool {
        self.temporary.load(Ordering::SeqCst)
    }

    fn set_temporary(&self, temporary: bool) {
        self.temporary.store(temporary, Ordering::SeqCst)
    }

    fn get_last_bootstrap_attempt(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt
    }

    fn set_last_bootstrap_attempt(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_bootstrap_attempt = instant;
    }

    fn get_last_packet_received(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_packet_received
    }

    fn set_last_packet_received(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_packet_received = instant;
    }

    fn get_last_packet_sent(&self) -> u64 {
        self.channel_mutex.lock().unwrap().last_packet_sent
    }

    fn set_last_packet_sent(&self, instant: u64) {
        self.channel_mutex.lock().unwrap().last_packet_sent = instant;
    }

    fn get_node_id(&self) -> Option<Account> {
        self.channel_mutex.lock().unwrap().node_id
    }

    fn set_node_id(&self, id: Account) {
        self.channel_mutex.lock().unwrap().node_id = Some(id);
    }

    fn is_alive(&self) -> bool {
        !self.closed.load(Ordering::SeqCst)
    }

    fn channel_id(&self) -> usize {
        self.channel_id
    }

    fn get_type(&self) -> super::TransportType {
        super::TransportType::Fake
    }
}
