use crate::{
    config::{NetworkConstants, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
    utils::{
        ipv4_address_or_ipv6_subnet, is_ipv4_mapped, map_address_to_subnetwork, reserved_address,
    },
};

use super::{
    attempt_container::AttemptContainer, AcceptResult, ChannelDirection, ChannelId, ChannelMode,
    PeerExclusion, TcpConfig, TrafficType,
};
use num::FromPrimitive;
use rand::{seq::SliceRandom, thread_rng};
use rsnano_core::{
    utils::{
        seconds_since_epoch, ContainerInfo, ContainerInfoComponent, TEST_ENDPOINT_1,
        TEST_ENDPOINT_2,
    },
    PublicKey,
};
use rsnano_messages::ProtocolInfo;
use std::{
    collections::HashMap,
    net::{Ipv6Addr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::debug;

/// Default timeout in seconds
const DEFAULT_TIMEOUT: u64 = 120;

pub struct ChannelInfo {
    channel_id: ChannelId,
    local_addr: SocketAddrV6,
    peer_addr: SocketAddrV6,
    data: Mutex<ChannelInfoData>,
    protocol_version: AtomicU8,
    direction: ChannelDirection,

    /// the timestamp (in seconds since epoch) of the last time there was successful activity on the socket
    last_activity: AtomicU64, // TODO use Timestamp

    /// Duration in seconds of inactivity that causes a socket timeout
    /// activity is any successful connect, send or receive event
    timeout_seconds: AtomicU64,

    /// Flag that is set when cleanup decides to close the socket due to timeout.
    /// NOTE: Currently used by tcp_server::timeout() but I suspect that this and tcp_server::timeout() are not needed.
    timed_out: AtomicBool,

    /// Set by close() - completion handlers must check this. This is more reliable than checking
    /// error codes as the OS may have already completed the async operation.
    closed: AtomicBool,

    socket_type: AtomicU8,
}

impl ChannelInfo {
    pub fn new(
        channel_id: ChannelId,
        local_addr: SocketAddrV6,
        peer_addr: SocketAddrV6,
        direction: ChannelDirection,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            channel_id,
            local_addr,
            peer_addr,
            // TODO set protocol version to 0
            protocol_version: AtomicU8::new(ProtocolInfo::default().version_using),
            direction,
            last_activity: AtomicU64::new(seconds_since_epoch()),
            timeout_seconds: AtomicU64::new(DEFAULT_TIMEOUT),
            timed_out: AtomicBool::new(false),
            socket_type: AtomicU8::new(ChannelMode::Undefined as u8),
            closed: AtomicBool::new(false),
            data: Mutex::new(ChannelInfoData {
                node_id: None,
                last_bootstrap_attempt: UNIX_EPOCH,
                last_packet_received: now,
                last_packet_sent: now,
                is_queue_full_impl: None,
                peering_addr: if direction == ChannelDirection::Outbound {
                    Some(peer_addr)
                } else {
                    None
                },
            }),
        }
    }

    pub fn new_test_instance() -> Self {
        Self::new(
            ChannelId::from(42),
            TEST_ENDPOINT_1,
            TEST_ENDPOINT_2,
            ChannelDirection::Outbound,
        )
    }

    pub fn set_queue_full_query(&self, query: Box<dyn Fn(TrafficType) -> bool + Send>) {
        self.data.lock().unwrap().is_queue_full_impl = Some(query);
    }

    pub fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    pub fn node_id(&self) -> Option<PublicKey> {
        self.data.lock().unwrap().node_id
    }

    pub fn direction(&self) -> ChannelDirection {
        self.direction
    }

    pub fn local_addr(&self) -> SocketAddrV6 {
        self.local_addr
    }

    /// The address that we are connected to. If this is an incoming channel, then
    /// the peer_addr uses an ephemeral port
    pub fn peer_addr(&self) -> SocketAddrV6 {
        self.peer_addr
    }

    /// The address where the peer accepts incoming connections. In case of an outbound
    /// channel, the peer_addr and peering_addr are the same
    pub fn peering_addr(&self) -> Option<SocketAddrV6> {
        self.data.lock().unwrap().peering_addr.clone()
    }

    pub fn peering_addr_or_peer_addr(&self) -> SocketAddrV6 {
        self.data
            .lock()
            .unwrap()
            .peering_addr
            .clone()
            .unwrap_or(self.peer_addr())
    }

    pub fn ipv4_address_or_ipv6_subnet(&self) -> Ipv6Addr {
        ipv4_address_or_ipv6_subnet(&self.peer_addr().ip())
    }

    pub fn subnetwork(&self) -> Ipv6Addr {
        map_address_to_subnetwork(self.peer_addr().ip())
    }

    pub fn protocol_version(&self) -> u8 {
        self.protocol_version.load(Ordering::Relaxed)
    }

    // TODO make private and set via NetworkInfo
    pub fn set_protocol_version(&self, version: u8) {
        self.protocol_version.store(version, Ordering::Relaxed);
    }

    pub fn last_activity(&self) -> u64 {
        self.last_activity.load(Ordering::Relaxed)
    }

    pub fn set_last_activity(&self, value: u64) {
        self.last_activity.store(value, Ordering::Relaxed)
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds.load(Ordering::Relaxed))
    }

    pub fn set_timeout(&self, value: Duration) {
        self.timeout_seconds
            .store(value.as_secs(), Ordering::Relaxed)
    }

    pub fn timed_out(&self) -> bool {
        self.timed_out.load(Ordering::Relaxed)
    }

    pub fn set_timed_out(&self, value: bool) {
        self.timed_out.store(value, Ordering::Relaxed)
    }

    pub fn is_alive(&self) -> bool {
        !self.is_closed()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.set_timeout(Duration::ZERO);
    }

    pub fn set_node_id(&self, node_id: PublicKey) {
        self.data.lock().unwrap().node_id = Some(node_id);
    }

    pub fn set_peering_addr(&self, peering_addr: SocketAddrV6) {
        self.data.lock().unwrap().peering_addr = Some(peering_addr);
    }

    pub fn mode(&self) -> ChannelMode {
        FromPrimitive::from_u8(self.socket_type.load(Ordering::SeqCst)).unwrap()
    }

    pub fn set_mode(&self, mode: ChannelMode) {
        self.socket_type.store(mode as u8, Ordering::SeqCst);
    }

    pub fn last_bootstrap_attempt(&self) -> SystemTime {
        self.data.lock().unwrap().last_bootstrap_attempt
    }

    pub fn set_last_bootstrap_attempt(&self, time: SystemTime) {
        self.data.lock().unwrap().last_bootstrap_attempt = time;
    }

    pub fn last_packet_received(&self) -> SystemTime {
        self.data.lock().unwrap().last_packet_received
    }

    pub fn set_last_packet_received(&self, instant: SystemTime) {
        self.data.lock().unwrap().last_packet_received = instant;
    }

    pub fn last_packet_sent(&self) -> SystemTime {
        self.data.lock().unwrap().last_packet_sent
    }

    pub fn set_last_packet_sent(&self, instant: SystemTime) {
        self.data.lock().unwrap().last_packet_sent = instant;
    }

    pub fn is_queue_full(&self, traffic_type: TrafficType) -> bool {
        let guard = self.data.lock().unwrap();
        match &guard.is_queue_full_impl {
            Some(cb) => cb(traffic_type),
            None => false,
        }
    }
}

struct ChannelInfoData {
    node_id: Option<PublicKey>,
    peering_addr: Option<SocketAddrV6>,
    last_bootstrap_attempt: SystemTime,
    last_packet_received: SystemTime,
    last_packet_sent: SystemTime,
    is_queue_full_impl: Option<Box<dyn Fn(TrafficType) -> bool + Send>>,
}

pub struct NetworkInfo {
    next_channel_id: usize,
    channels: HashMap<ChannelId, Arc<ChannelInfo>>,
    listening_port: u16,
    stopped: bool,
    new_realtime_channel_observers: Vec<Arc<dyn Fn(Arc<ChannelInfo>) + Send + Sync>>,
    stats: Arc<Stats>,
    pub attempts: AttemptContainer,
    protocol: ProtocolInfo,
    tcp_config: TcpConfig,
    node_flags: NodeFlags,
    network_constants: NetworkConstants,
    pub(crate) excluded_peers: PeerExclusion,
}

impl NetworkInfo {
    pub fn new(
        listening_port: u16,
        protocol: ProtocolInfo,
        tcp_config: TcpConfig,
        stats: Arc<Stats>,
        node_flags: NodeFlags,
        network_constants: NetworkConstants,
    ) -> Self {
        Self {
            next_channel_id: 1,
            channels: HashMap::new(),
            listening_port,
            stopped: false,
            new_realtime_channel_observers: Vec::new(),
            stats,
            attempts: Default::default(),
            protocol,
            tcp_config,
            excluded_peers: PeerExclusion::new(),
            node_flags,
            network_constants,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new_test_instance() -> Self {
        Self::new(
            8080,
            Default::default(),
            Default::default(),
            Arc::new(Stats::default()),
            Default::default(),
            Default::default(),
        )
    }

    pub(crate) fn on_new_realtime_channel(
        &mut self,
        callback: Arc<dyn Fn(Arc<ChannelInfo>) + Send + Sync>,
    ) {
        self.new_realtime_channel_observers.push(callback);
    }

    pub(crate) fn new_realtime_channel_observers(
        &self,
    ) -> Vec<Arc<dyn Fn(Arc<ChannelInfo>) + Send + Sync>> {
        self.new_realtime_channel_observers.clone()
    }

    pub(crate) fn add_attempt(&mut self, remote: SocketAddrV6) -> bool {
        let count = self.attempts.count_by_address(remote.ip());
        if count >= self.tcp_config.max_attempts_per_ip {
            self.stats.inc_dir(
                StatType::TcpListenerRejected,
                DetailType::MaxAttemptsPerIp,
                Direction::Out,
            );
            debug!("Connection attempt already in progress ({}), unable to initiate new connection: {}", count, remote.ip());
            return false; // Rejected
        }

        self.attempts.insert(remote, ChannelDirection::Outbound)
    }

    pub(crate) fn remove_attempt(&mut self, remote: &SocketAddrV6) {
        self.attempts.remove(&remote);
    }

    pub fn add(
        &mut self,
        local_addr: SocketAddrV6,
        peer_addr: SocketAddrV6,
        direction: ChannelDirection,
    ) -> Arc<ChannelInfo> {
        let channel_id = self.get_next_channel_id();
        let channel_info = Arc::new(ChannelInfo::new(
            channel_id, local_addr, peer_addr, direction,
        ));
        self.channels.insert(channel_id, channel_info.clone());
        channel_info
    }

    fn get_next_channel_id(&mut self) -> ChannelId {
        let id = self.next_channel_id.into();
        self.next_channel_id += 1;
        id
    }

    pub fn listening_port(&self) -> u16 {
        self.listening_port
    }

    pub fn set_listening_port(&mut self, port: u16) {
        self.listening_port = port
    }

    pub fn get(&self, channel_id: ChannelId) -> Option<&Arc<ChannelInfo>> {
        self.channels.get(&channel_id)
    }

    pub fn remove(&mut self, channel_id: ChannelId) {
        self.channels.remove(&channel_id);
    }

    pub fn set_node_id(&self, channel_id: ChannelId, node_id: PublicKey) {
        if let Some(channel) = self.channels.get(&channel_id) {
            channel.set_node_id(node_id);
        }
    }

    pub fn find_node_id(&self, node_id: &PublicKey) -> Option<&Arc<ChannelInfo>> {
        self.channels
            .values()
            .find(|c| c.node_id() == Some(*node_id))
    }

    pub fn find_realtime_channel_by_remote_addr(
        &self,
        endpoint: &SocketAddrV6,
    ) -> Option<&Arc<ChannelInfo>> {
        self.channels.values().find(|c| {
            c.mode() == ChannelMode::Realtime && c.is_alive() && c.peer_addr() == *endpoint
        })
    }

    pub(crate) fn find_realtime_channel_by_peering_addr(
        &self,
        peering_addr: &SocketAddrV6,
    ) -> Option<ChannelId> {
        self.channels
            .values()
            .find(|c| {
                c.mode() == ChannelMode::Realtime
                    && c.is_alive()
                    && c.peering_addr() == Some(*peering_addr)
            })
            .map(|c| c.channel_id())
    }

    pub fn random_realtime_channels(&self, count: usize, min_version: u8) -> Vec<Arc<ChannelInfo>> {
        let mut channels = self.list_realtime(min_version);
        let mut rng = thread_rng();
        channels.shuffle(&mut rng);
        if count > 0 {
            channels.truncate(count)
        }
        channels
    }

    pub fn list_realtime(&self, min_version: u8) -> Vec<Arc<ChannelInfo>> {
        self.channels
            .values()
            .filter(|c| {
                c.protocol_version() >= min_version
                    && c.is_alive()
                    && c.mode() == ChannelMode::Realtime
            })
            .map(|c| c.clone())
            .collect()
    }

    pub(crate) fn list_realtime_channels(&self, min_version: u8) -> Vec<Arc<ChannelInfo>> {
        let mut result = self.list_realtime(min_version);
        result.sort_by_key(|i| i.peer_addr());
        result
    }

    pub fn not_a_peer(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        endpoint.ip().is_unspecified()
            || reserved_address(endpoint, allow_local_peers)
            || endpoint == &SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.listening_port(), 0, 0)
    }

    pub(crate) fn random_list_realtime(
        &self,
        count: usize,
        min_version: u8,
    ) -> Vec<Arc<ChannelInfo>> {
        let mut channels = self.list_realtime(min_version);
        let mut rng = thread_rng();
        channels.shuffle(&mut rng);
        if count > 0 {
            channels.truncate(count)
        }
        channels
    }

    pub(crate) fn random_list_realtime_ids(&self) -> Vec<ChannelId> {
        self.random_list_realtime(usize::MAX, 0)
            .iter()
            .map(|c| c.channel_id())
            .collect()
    }

    pub fn purge(&mut self, cutoff: SystemTime) -> Vec<ChannelId> {
        self.close_idle_channels(cutoff);

        // Check if any tcp channels belonging to old protocol versions which may still be alive due to async operations
        self.close_old_protocol_versions(self.protocol.version_min);

        // Remove channels with dead underlying sockets
        let purged_channel_ids = self.remove_dead_channels();

        // Remove keepalive attempt tracking for attempts older than cutoff
        self.attempts.purge(cutoff);
        purged_channel_ids
    }

    fn close_idle_channels(&mut self, cutoff: SystemTime) {
        for entry in self.channels.values() {
            if entry.last_packet_sent() < cutoff {
                debug!(remote_addr = ?entry.peer_addr(), channel_id = %entry.channel_id(), mode = ?entry.mode(), "Closing idle channel");
                entry.close();
            }
        }
    }

    fn close_old_protocol_versions(&mut self, min_version: u8) {
        for channel in self.channels.values() {
            if channel.protocol_version() < min_version {
                debug!(channel_id = %channel.channel_id(), peer_addr = ?channel.peer_addr(), version = channel.protocol_version(), min_version,
                    "Closing channel with old protocol version",
                );
                channel.close();
            }
        }
    }

    /// Removes dead channels and returns their channel ids
    fn remove_dead_channels(&mut self) -> Vec<ChannelId> {
        let dead_channels: Vec<_> = self
            .channels
            .values()
            .filter(|c| !c.is_alive())
            .cloned()
            .collect();

        for channel in &dead_channels {
            debug!("Removing dead channel: {}", channel.peer_addr());
            self.channels.remove(&channel.channel_id());
        }

        dead_channels.iter().map(|c| c.channel_id()).collect()
    }

    pub fn check_limits(
        &mut self,
        peer: &SocketAddrV6,
        direction: ChannelDirection,
    ) -> AcceptResult {
        if !self.node_flags.disable_max_peers_per_ip {
            let count = self.count_by_ip(peer.ip());
            if count >= self.network_constants.max_peers_per_ip {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerIp,
                    direction.into(),
                );
                debug!(
                    "Max connections per IP reached ({}, count: {}), unable to open new connection",
                    peer.ip(),
                    count
                );
                return AcceptResult::Rejected;
            }
        }

        // If the address is IPv4 we don't check for a network limit, since its address space isn't big as IPv6/64.
        if !self.node_flags.disable_max_peers_per_subnetwork && !is_ipv4_mapped(&peer.ip()) {
            let subnet = map_address_to_subnetwork(&peer.ip());
            let count = self.count_by_subnet(&subnet);
            if count >= self.network_constants.max_peers_per_subnetwork {
                self.stats.inc_dir(
                    StatType::TcpListenerRejected,
                    DetailType::MaxPerSubnetwork,
                    direction.into(),
                );
                debug!(
                    "Max connections per subnetwork reached ({}), unable to open new connection",
                    peer.ip()
                );
                return AcceptResult::Rejected;
            }
        }

        match direction {
            ChannelDirection::Inbound => {
                let count = self.count_by_direction(ChannelDirection::Inbound);

                if count >= self.tcp_config.max_inbound_connections {
                    self.stats.inc_dir(
                        StatType::TcpListenerRejected,
                        DetailType::MaxAttempts,
                        direction.into(),
                    );
                    debug!(
                        "Max inbound connections reached ({}), unable to accept new connection: {}",
                        count,
                        peer.ip()
                    );
                    return AcceptResult::Rejected;
                }
            }
            ChannelDirection::Outbound => {
                let count = self.count_by_direction(ChannelDirection::Outbound);

                if count >= self.tcp_config.max_outbound_connections {
                    self.stats.inc_dir(
                        StatType::TcpListenerRejected,
                        DetailType::MaxAttempts,
                        direction.into(),
                    );
                    debug!(
                        "Max outbound connections reached ({}), unable to initiate new connection: {}",
                        count, peer.ip()
                    );
                    return AcceptResult::Rejected;
                }
            }
        }

        AcceptResult::Accepted
    }

    fn count_by_ip(&self, ip: &Ipv6Addr) -> usize {
        self.channels
            .values()
            .filter(|c| c.is_alive() && c.ipv4_address_or_ipv6_subnet() == *ip)
            .count()
    }

    fn count_by_subnet(&self, subnet: &Ipv6Addr) -> usize {
        self.channels
            .values()
            .filter(|c| c.is_alive() && c.subnetwork() == *subnet)
            .count()
    }

    fn count_by_direction(&self, direction: ChannelDirection) -> usize {
        self.channels
            .values()
            .filter(|c| c.is_alive() && c.direction() == direction)
            .count()
    }

    pub fn stop(&mut self) -> bool {
        if self.stopped {
            false
        } else {
            self.stopped = true;
            true
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "channels".to_string(),
                    count: self.channels.len(),
                    sizeof_element: size_of::<Arc<ChannelInfo>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "attempts".to_string(),
                    count: self.attempts.len(),
                    sizeof_element: AttemptContainer::ELEMENT_SIZE,
                }),
                self.excluded_peers.collect_container_info("excluded_peers"),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_ip_is_not_a_peer() {
        let network = NetworkInfo::new_test_instance();

        assert!(network.not_a_peer(
            &SocketAddrV6::new(Ipv6Addr::new(0xff00u16, 0, 0, 0, 0, 0, 0, 0), 1000, 0, 0),
            true
        ));
        assert!(network.not_a_peer(&SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 10000, 0, 0), true));
        assert!(network.not_a_peer(
            &SocketAddrV6::new(Ipv6Addr::LOCALHOST, network.listening_port(), 0, 0),
            false
        ));

        // Test with a valid IP address
        assert_eq!(
            network.not_a_peer(
                &SocketAddrV6::new(Ipv6Addr::from_bits(0x08080808), 10000, 0, 0),
                true
            ),
            false
        );
    }
}
