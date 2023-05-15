use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

use message_deserializer::MessageDeserializer;
use rsnano_core::Account;

use crate::{
    config::NetworkConstants,
    messages::Message,
    transport::message_deserializer,
    utils::{BlockUniquer, ErrorCode},
    voting::VoteUniquer,
};

use super::{message_deserializer::ReadQuery, Channel, MessageDeserializerExt, NetworkFilter};

pub struct InProcChannelData {
    last_bootstrap_attempt: u64,
    last_packet_received: u64,
    last_packet_sent: u64,
    node_id: Option<Account>,
}

pub struct ChannelInProc {
    channel_id: usize,
    temporary: AtomicBool,
    channel_mutex: Mutex<InProcChannelData>,
    network_constants: NetworkConstants,
    network_filter: Arc<NetworkFilter>,
    block_uniquer: Arc<BlockUniquer>,
    vote_uniquer: Arc<VoteUniquer>,
}

impl ChannelInProc {
    pub fn new(
        channel_id: usize,
        now: u64,
        network_constants: NetworkConstants,
        network_filter: Arc<NetworkFilter>,
        block_uniquer: Arc<BlockUniquer>,
        vote_uniquer: Arc<VoteUniquer>,
    ) -> Self {
        Self {
            channel_id,
            temporary: AtomicBool::new(false),
            channel_mutex: Mutex::new(InProcChannelData {
                last_bootstrap_attempt: 0,
                last_packet_received: now,
                last_packet_sent: now,
                node_id: None,
            }),
            network_constants,
            network_filter,
            block_uniquer,
            vote_uniquer,
        }
    }

    pub fn send_buffer(
        &self,
        buffer: &[u8],
        callback_msg: Box<dyn FnOnce(ErrorCode, Option<Box<dyn Message>>)>,
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
            self.block_uniquer.clone(),
            self.vote_uniquer.clone(),
            buffer_read_fn,
        ));
        message_deserializer.read(callback_msg);
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
        true
    }

    fn channel_id(&self) -> usize {
        self.channel_id
    }
}
