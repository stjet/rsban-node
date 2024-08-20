use super::{
    Channel, ChannelDirection, ChannelId, ChannelMode, DropPolicy, NetworkFilter, NetworkInfo,
    OutboundBandwidthLimiter, TcpStream, TrafficType,
};
use crate::{
    stats::Stats,
    utils::{into_ipv6_socket_address, SteadyClock},
    NetworkParams, DEV_NETWORK_PARAMS,
};
use rsnano_core::utils::NULL_ENDPOINT;
use std::{
    collections::HashMap,
    net::SocketAddrV6,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime},
};
use tracing::{debug, warn};

pub struct NetworkOptions {
    pub publish_filter: Arc<NetworkFilter>,
    pub network_params: NetworkParams,
    pub stats: Arc<Stats>,
    pub limiter: Arc<OutboundBandwidthLimiter>,
    pub clock: Arc<SteadyClock>,
    pub network_info: Arc<RwLock<NetworkInfo>>,
}

impl NetworkOptions {
    pub fn new_test_instance() -> Self {
        NetworkOptions {
            publish_filter: Arc::new(NetworkFilter::default()),
            network_params: DEV_NETWORK_PARAMS.clone(),
            stats: Arc::new(Default::default()),
            limiter: Arc::new(OutboundBandwidthLimiter::default()),
            clock: Arc::new(SteadyClock::new_null()),
            network_info: Arc::new(RwLock::new(NetworkInfo::new_test_instance())),
        }
    }
}

pub struct Network {
    channels: Mutex<HashMap<ChannelId, Arc<Channel>>>,
    pub info: Arc<RwLock<NetworkInfo>>,
    stats: Arc<Stats>,
    network_params: Arc<NetworkParams>,
    limiter: Arc<OutboundBandwidthLimiter>,
    pub publish_filter: Arc<NetworkFilter>,
    clock: Arc<SteadyClock>,
}

impl Network {
    pub fn new(options: NetworkOptions) -> Self {
        let network = Arc::new(options.network_params);

        Self {
            channels: Mutex::new(HashMap::new()),
            stats: options.stats,
            network_params: network,
            limiter: options.limiter,
            publish_filter: options.publish_filter,
            clock: options.clock,
            info: options.network_info,
        }
    }

    pub(crate) fn channels_info(&self) -> ChannelsInfo {
        self.info.read().unwrap().channels_info()
    }

    pub(crate) async fn wait_for_available_inbound_slot(&self) {
        let last_log = Instant::now();
        let log_interval = if self.network_params.network.is_dev_network() {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(15)
        };
        while {
            let info = self.info.read().unwrap();
            !info.is_inbound_slot_available() && !info.is_stopped()
        } {
            if last_log.elapsed() >= log_interval {
                warn!("Waiting for available slots to accept new connections");
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub fn can_add_connection(
        &self,
        peer_addr: &SocketAddrV6,
        direction: ChannelDirection,
        planned_mode: ChannelMode,
    ) -> AcceptResult {
        self.info.write().unwrap().can_add_connection(
            peer_addr,
            direction,
            planned_mode,
            self.clock.now(),
        )
    }

    pub async fn add(
        &self,
        stream: TcpStream,
        direction: ChannelDirection,
        planned_mode: ChannelMode,
    ) -> anyhow::Result<Arc<Channel>> {
        let peer_addr = stream
            .peer_addr()
            .map(into_ipv6_socket_address)
            .unwrap_or(NULL_ENDPOINT);

        let local_addr = stream
            .local_addr()
            .map(into_ipv6_socket_address)
            .unwrap_or(NULL_ENDPOINT);

        let channel_info = self.info.write().unwrap().add(
            local_addr,
            peer_addr,
            direction,
            planned_mode,
            self.clock.now(),
        )?;

        let channel = Channel::create(
            channel_info,
            stream,
            self.stats.clone(),
            self.limiter.clone(),
            self.info.clone(),
        )
        .await;
        self.channels
            .lock()
            .unwrap()
            .insert(channel.channel_id(), channel.clone());

        debug!(?peer_addr, ?direction, "Accepted connection");

        Ok(channel)
    }

    pub(crate) fn new_null() -> Self {
        Self::new(NetworkOptions::new_test_instance())
    }

    pub(crate) fn add_attempt(&self, remote: SocketAddrV6) -> bool {
        self.info.write().unwrap().add_attempt(remote)
    }

    pub(crate) fn remove_attempt(&self, remote: &SocketAddrV6) {
        self.info.write().unwrap().remove_attempt(remote)
    }

    pub fn random_fill_peering_endpoints(&self, endpoints: &mut [SocketAddrV6]) {
        self.info.read().unwrap().random_fill_realtime(endpoints);
    }

    pub(crate) fn try_send_buffer(
        &self,
        channel_id: ChannelId,
        buffer: &[u8],
        drop_policy: DropPolicy,
        traffic_type: TrafficType,
    ) -> bool {
        if let Some(channel) = self.channels.lock().unwrap().get(&channel_id).cloned() {
            channel.try_send_buffer(buffer, drop_policy, traffic_type)
        } else {
            false
        }
    }

    pub async fn send_buffer(
        &self,
        channel_id: ChannelId,
        buffer: &[u8],
        traffic_type: TrafficType,
    ) -> anyhow::Result<()> {
        let channel = self.channels.lock().unwrap().get(&channel_id).cloned();

        if let Some(channel) = channel {
            channel.send_buffer(buffer, traffic_type).await
        } else {
            Err(anyhow!("Channel not found"))
        }
    }

    /// Returns channel IDs of removed channels
    pub fn purge(&self, cutoff: SystemTime) -> Vec<ChannelId> {
        let channel_ids = self.info.write().unwrap().purge(cutoff);
        let mut guard = self.channels.lock().unwrap();
        for channel_id in &channel_ids {
            guard.remove(channel_id);
        }
        channel_ids
    }

    pub fn count_by_mode(&self, mode: ChannelMode) -> usize {
        self.info.read().unwrap().count_by_mode(mode)
    }

    pub fn port(&self) -> u16 {
        self.info.read().unwrap().listening_port()
    }

    pub(crate) fn is_excluded(&self, addr: &SocketAddrV6) -> bool {
        self.info
            .write()
            .unwrap()
            .excluded_peers
            .is_excluded(addr, self.clock.now())
    }

    pub(crate) fn perma_ban(&self, remote_addr: SocketAddrV6) {
        self.info
            .write()
            .unwrap()
            .excluded_peers
            .perma_ban(remote_addr);
    }

    pub(crate) fn keepalive_list(&self) -> Vec<ChannelId> {
        self.info.read().unwrap().keepalive_list()
    }
}

#[derive(PartialEq, Eq)]
pub enum AcceptResult {
    Invalid,
    Accepted,
    Rejected,
    Error,
}

#[derive(Default)]
pub(crate) struct ChannelsInfo {
    pub total: usize,
    pub realtime: usize,
    pub bootstrap: usize,
    pub inbound: usize,
    pub outbound: usize,
}
