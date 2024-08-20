use super::{
    write_queue::{WriteQueue, WriteQueueReceiver},
    AsyncBufferReader, ChannelDirection, ChannelId, ChannelInfo, DropPolicy, NetworkInfo,
    OutboundBandwidthLimiter, TcpStream, TrafficType,
};
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    utils::{into_ipv6_socket_address, ipv4_address_or_ipv6_subnet, map_address_to_subnetwork},
};
use async_trait::async_trait;
use rsnano_core::{
    utils::{seconds_since_epoch, TEST_ENDPOINT_1},
    Account,
};
use std::{
    fmt::Display,
    net::{Ipv6Addr, SocketAddrV6},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::sleep;
use tracing::debug;

pub struct ChannelData {
    last_packet_received: SystemTime,
    last_packet_sent: SystemTime,
}

pub struct Channel {
    channel_id: ChannelId,
    network_info: Arc<RwLock<NetworkInfo>>,
    pub info: Arc<ChannelInfo>,
    channel_mutex: Mutex<ChannelData>,
    limiter: Arc<OutboundBandwidthLimiter>,
    stats: Arc<Stats>,
    write_queue: WriteQueue,
    stream: Arc<TcpStream>,
    ignore_closed_write_queue: bool,
}

impl Channel {
    const MAX_QUEUE_SIZE: usize = 128;

    fn new(
        channel_info: Arc<ChannelInfo>,
        network_info: Arc<RwLock<NetworkInfo>>,
        stream: Arc<TcpStream>,
        stats: Arc<Stats>,
        limiter: Arc<OutboundBandwidthLimiter>,
    ) -> (Self, WriteQueueReceiver) {
        let (write_queue, receiver) = WriteQueue::new(Self::MAX_QUEUE_SIZE);

        let now = SystemTime::now();
        let channel = Self {
            channel_id: channel_info.channel_id(),
            info: channel_info,
            network_info,
            channel_mutex: Mutex::new(ChannelData {
                last_packet_received: now,
                last_packet_sent: now,
            }),
            limiter,
            stats,
            write_queue,
            stream,
            ignore_closed_write_queue: false,
        };

        (channel, receiver)
    }

    pub fn new_null() -> Self {
        Self::new_null_with_id(42)
    }

    pub fn new_null_with_id(id: impl Into<ChannelId>) -> Self {
        let channel_id = id.into();
        let (mut channel, _receiver) = Self::new(
            Arc::new(ChannelInfo::new(
                channel_id,
                TEST_ENDPOINT_1,
                ChannelDirection::Outbound,
            )),
            Arc::new(RwLock::new(NetworkInfo::new())),
            Arc::new(TcpStream::new_null()),
            Arc::new(Stats::default()),
            Arc::new(OutboundBandwidthLimiter::default()),
        );
        // We drop the write queue receiver, so the channel would be dead immediately.
        channel.ignore_closed_write_queue = true;
        channel
    }

    pub async fn create(
        channel_info: Arc<ChannelInfo>,
        stream: TcpStream,
        stats: Arc<Stats>,
        limiter: Arc<OutboundBandwidthLimiter>,
        network_info: Arc<RwLock<NetworkInfo>>,
    ) -> Arc<Self> {
        let stream = Arc::new(stream);
        let stream_l = stream.clone();
        let (channel, mut receiver) = Self::new(channel_info, network_info, stream, stats, limiter);
        //
        // process write queue:
        tokio::spawn(async move {
            while let Some(entry) = receiver.pop().await {
                let mut written = 0;
                let buffer = &entry.buffer;
                loop {
                    match stream_l.writable().await {
                        Ok(()) => match stream_l.try_write(&buffer[written..]) {
                            Ok(n) => {
                                written += n;
                                if written >= buffer.len() {
                                    break;
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(_) => {
                                break;
                            }
                        },
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
        });

        let channel = Arc::new(channel);
        let channel_l = channel.clone();
        tokio::spawn(async move { channel_l.ongoing_checkup().await });
        channel
    }

    pub(crate) fn set_peering_addr(&self, address: SocketAddrV6) {
        self.network_info
            .read()
            .unwrap()
            .set_peering_addr(self.channel_id(), address);
    }

    pub(crate) fn is_queue_full(&self, traffic_type: TrafficType) -> bool {
        self.write_queue.capacity(traffic_type) <= Self::MAX_QUEUE_SIZE
    }

    fn is_closed(&self) -> bool {
        self.info.is_closed() || (!self.ignore_closed_write_queue && self.write_queue.is_closed())
    }

    fn update_last_activity(&self) {
        self.info.set_last_activity(seconds_since_epoch());
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    pub fn get_last_bootstrap_attempt(&self) -> SystemTime {
        self.info.last_bootstrap_attempt()
    }

    pub fn set_last_bootstrap_attempt(&self, time: SystemTime) {
        self.info.set_last_bootstrap_attempt(time);
    }

    pub fn get_last_packet_received(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_received
    }

    pub fn set_last_packet_received(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_received = instant;
    }

    pub fn get_last_packet_sent(&self) -> SystemTime {
        self.channel_mutex.lock().unwrap().last_packet_sent
    }

    pub fn set_last_packet_sent(&self, instant: SystemTime) {
        self.channel_mutex.lock().unwrap().last_packet_sent = instant;
    }

    pub fn set_node_id(&self, id: Account) {
        self.network_info
            .read()
            .unwrap()
            .set_node_id(self.channel_id, id);
    }

    pub fn is_alive(&self) -> bool {
        !self.is_closed()
    }

    pub fn local_addr(&self) -> SocketAddrV6 {
        self.stream
            .local_addr()
            .map(|addr| into_ipv6_socket_address(addr))
            .unwrap_or(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0))
    }

    pub async fn send_buffer(
        &self,
        buffer: &[u8],
        traffic_type: TrafficType,
    ) -> anyhow::Result<()> {
        while self.is_queue_full(traffic_type) {
            // TODO: better implementation
            sleep(Duration::from_millis(20)).await;
        }

        while !self.limiter.should_pass(buffer.len(), traffic_type.into()) {
            // TODO: better implementation
            sleep(Duration::from_millis(20)).await;
        }

        if self.is_closed() {
            bail!("socket closed");
        }

        let buf_size = buffer.len();

        let result = self
            .write_queue
            .insert(Arc::new(buffer.to_vec()), traffic_type) // TODO don't copy into vec. Split into fixed size packets
            .await;

        if result.is_ok() {
            self.stats.add_dir_aggregate(
                StatType::TrafficTcp,
                DetailType::All,
                Direction::Out,
                buf_size as u64,
            );
            self.update_last_activity();
            self.set_last_packet_sent(SystemTime::now());
        } else {
            self.stats
                .inc_dir(StatType::Tcp, DetailType::TcpWriteError, Direction::In);
            debug!(channel_id = %self.channel_id(), remote_addr = ?self.info.peer_addr(), "Closing channel after write error");
            self.info.close();
        }

        result?;

        self.channel_mutex.lock().unwrap().last_packet_sent = SystemTime::now();
        Ok(())
    }

    pub fn try_send_buffer(
        &self,
        buffer: &[u8],
        drop_policy: DropPolicy,
        traffic_type: TrafficType,
    ) -> bool {
        if self.is_closed() {
            return false;
        }

        if drop_policy == DropPolicy::CanDrop && self.is_queue_full(traffic_type) {
            return false;
        }

        let should_pass = self.limiter.should_pass(buffer.len(), traffic_type.into());
        if !should_pass && drop_policy == DropPolicy::CanDrop {
            return false;
        } else {
            // TODO notify bandwidth limiter that we are sending it anyway
        }

        let buf_size = buffer.len();

        let (inserted, write_error) = self
            .write_queue
            .try_insert(Arc::new(buffer.to_vec()), traffic_type); // TODO don't copy into vec. Split into fixed size packets

        if inserted {
            self.stats.add_dir_aggregate(
                StatType::TrafficTcp,
                DetailType::All,
                Direction::Out,
                buf_size as u64,
            );
            self.update_last_activity();
            self.set_last_packet_sent(SystemTime::now());
        } else if write_error {
            self.stats
                .inc_dir(StatType::Tcp, DetailType::TcpWriteError, Direction::In);
            self.info.close();
            debug!(peer_addr = ?self.info.peer_addr(), channel_id = %self.channel_id(), mode = ?self.info.mode(), "Closing socket after write error");
        }
        inserted
    }

    pub fn ipv4_address_or_ipv6_subnet(&self) -> Ipv6Addr {
        ipv4_address_or_ipv6_subnet(&self.info.peer_addr().ip())
    }

    pub fn subnetwork(&self) -> Ipv6Addr {
        map_address_to_subnetwork(self.info.peer_addr().ip())
    }

    async fn ongoing_checkup(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;
            // If the socket is already dead, close just in case, and stop doing checkups
            if !self.is_alive() {
                debug!(
                    peer_addr = ?self.info.peer_addr(),
                    "Stopping checkup for dead channel"
                );
                return;
            }

            let now = seconds_since_epoch();
            let mut condition_to_disconnect = false;

            // if there is no activity for timeout seconds then disconnect
            if (now - self.info.last_activity()) > self.info.timeout().as_secs() {
                self.stats.inc_dir(
                    StatType::Tcp,
                    DetailType::TcpIoTimeoutDrop,
                    if self.info.direction() == ChannelDirection::Inbound {
                        Direction::In
                    } else {
                        Direction::Out
                    },
                );
                condition_to_disconnect = true;
            }

            if condition_to_disconnect {
                debug!(channel_id = %self.channel_id(), remote_addr = ?self.info.peer_addr(), mode = ?self.info.mode(), direction = ?self.info.direction(), "Closing channel due to timeout");
                self.info.set_timed_out(true);
                self.info.close();
            }
        }
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.info.peer_addr().fmt(f)
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.info.close();
    }
}

#[async_trait]
impl AsyncBufferReader for Arc<Channel> {
    async fn read(&self, buffer: &mut [u8], count: usize) -> anyhow::Result<()> {
        if count > buffer.len() {
            return Err(anyhow!("buffer is too small for read count"));
        }

        if self.is_closed() {
            return Err(anyhow!("Tried to read from a closed TcpStream"));
        }

        let mut read = 0;
        loop {
            match self.stream.readable().await {
                Ok(_) => {
                    match self.stream.try_read(&mut buffer[read..count]) {
                        Ok(0) => {
                            self.stats.inc_dir(
                                StatType::Tcp,
                                DetailType::TcpReadError,
                                Direction::In,
                            );
                            return Err(anyhow!("remote side closed the channel"));
                        }
                        Ok(n) => {
                            read += n;
                            if read >= count {
                                self.stats.add_dir(
                                    StatType::TrafficTcp,
                                    DetailType::All,
                                    Direction::In,
                                    count as u64,
                                );
                                self.update_last_activity();
                                self.set_last_packet_received(SystemTime::now());
                                return Ok(());
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            self.stats.inc_dir(
                                StatType::Tcp,
                                DetailType::TcpReadError,
                                Direction::In,
                            );
                            return Err(e.into());
                        }
                    };
                }
                Err(e) => {
                    self.stats
                        .inc_dir(StatType::Tcp, DetailType::TcpReadError, Direction::In);
                    return Err(e.into());
                }
            }
        }
    }
}
