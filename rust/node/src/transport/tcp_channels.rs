use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::Hash,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{
        atomic::{AtomicBool, AtomicU16, Ordering},
        Arc, Mutex,
    },
    time::SystemTime,
};

use rand::{thread_rng, Rng};
use rsnano_core::PublicKey;

use crate::{
    bootstrap::ChannelTcpWrapper,
    config::NetworkConstants,
    utils::{ipv4_address_or_ipv6_subnet, map_address_to_subnetwork, reserved_address},
};

use super::{ChannelEnum, Socket, SocketImpl, TcpServer};

pub struct TcpChannels {
    pub tcp_channels: Mutex<TcpChannelsImpl>,
    pub port: AtomicU16,
    pub stopped: AtomicBool,
    allow_local_peers: bool,
}

impl TcpChannels {
    pub fn new(port: u16, network_constants: NetworkConstants, allow_local_peers: bool) -> Self {
        Self {
            tcp_channels: Mutex::new(TcpChannelsImpl::new(network_constants)),
            port: AtomicU16::new(port),
            stopped: AtomicBool::new(false),
            allow_local_peers,
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub fn not_a_peer(&self, endpoint: &SocketAddrV6, allow_local_peers: bool) -> bool {
        endpoint.ip().is_unspecified()
            || reserved_address(endpoint, allow_local_peers)
            || endpoint
                == &SocketAddrV6::new(Ipv6Addr::LOCALHOST, self.port.load(Ordering::SeqCst), 0, 0)
    }

    pub fn on_new_channel(&self, callback: Arc<dyn Fn(Arc<ChannelEnum>)>) {
        self.tcp_channels.lock().unwrap().new_channel_observer = Some(callback);
    }

    pub fn insert(
        &self,
        channel: &Arc<ChannelEnum>,
        socket: &Arc<SocketImpl>,
        server: Option<Arc<TcpServer>>,
    ) -> Result<(), ()> {
        let ChannelEnum::Tcp(tcp_channel) = channel.as_ref() else { panic!("not a tcp channel")};
        let endpoint = tcp_channel.endpoint();
        let SocketAddr::V6(endpoint_v6) = endpoint else {panic!("not a v6 address")};
        if !self.not_a_peer(&endpoint_v6, self.allow_local_peers)
            && !self.stopped.load(Ordering::SeqCst)
        {
            let mut lock = self.tcp_channels.lock().unwrap();
            if !lock.channels.exists(&endpoint) {
                let node_id = channel.as_channel().get_node_id().unwrap_or_default();
                if !channel.as_channel().is_temporary() {
                    lock.channels.remove_by_node_id(&node_id);
                }

                let wrapper = Arc::new(ChannelTcpWrapper::new(
                    channel.clone(),
                    socket.clone(),
                    server,
                ));
                lock.channels.insert(wrapper);
                lock.attempts.remove(&endpoint_v6);
                let observer = lock.new_channel_observer.clone();
                drop(lock);
                if let Some(callback) = observer {
                    callback(channel.clone());
                }
                return Ok(());
            }
        }
        Err(())
    }

    pub fn find_channel(&self, endpoint: &SocketAddr) -> Option<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().find_channel(endpoint)
    }

    pub fn random_channels(
        &self,
        count: usize,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        self.tcp_channels.lock().unwrap().random_channels(
            count,
            min_version,
            include_temporary_channels,
        )
    }
}

pub struct TcpChannelsImpl {
    pub attempts: TcpEndpointAttemptContainer,
    pub channels: ChannelContainer,
    network_constants: NetworkConstants,
    new_channel_observer: Option<Arc<dyn Fn(Arc<ChannelEnum>)>>,
}

impl TcpChannelsImpl {
    pub fn new(network_constants: NetworkConstants) -> Self {
        Self {
            attempts: Default::default(),
            channels: Default::default(),
            network_constants,
            new_channel_observer: None,
        }
    }

    pub fn bootstrap_peer(&mut self) -> SocketAddr {
        let mut channel_endpoint = None;
        let mut peering_endpoint = None;
        for channel in self.channels.iter_by_last_bootstrap_attempt() {
            if channel.network_version() >= self.network_constants.protocol_version_min {
                if let ChannelEnum::Tcp(tcp) = channel.channel.as_ref() {
                    channel_endpoint = Some(channel.endpoint());
                    peering_endpoint = Some(tcp.peering_endpoint());
                    break;
                }
            }
        }

        match (channel_endpoint, peering_endpoint) {
            (Some(ep), Some(peering)) => {
                self.channels
                    .set_last_bootstrap_attempt(&ep, SystemTime::now());
                peering
            }
            _ => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        }
    }

    pub fn close_channels(&mut self) {
        for channel in self.channels.iter() {
            if let Some(socket) = channel.socket() {
                socket.close();
            }
            // Remove response server
            if let Some(server) = &channel.response_server {
                server.stop();
            }
        }
        self.channels.clear();
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        // Remove channels with dead underlying sockets
        self.channels.remove_dead();
        self.channels.purge(cutoff);

        // Remove keepalive attempt tracking for attempts older than cutoff
        self.attempts.purge(cutoff);

        // Check if any tcp channels belonging to old protocol versions which may still be alive due to async operations
        self.channels
            .remove_old_protocol_versions(self.network_constants.protocol_version_min);
    }

    pub fn list(&self, min_version: u8, include_temporary_channels: bool) -> Vec<Arc<ChannelEnum>> {
        self.channels
            .iter()
            .filter(|c| {
                c.tcp_channel().network_version() >= min_version
                    && (include_temporary_channels || !c.channel.as_channel().is_temporary())
            })
            .map(|c| c.channel.clone())
            .collect()
    }

    pub fn keepalive_list(&self) -> Vec<Arc<ChannelEnum>> {
        let cutoff = SystemTime::now() - self.network_constants.keepalive_period;
        let mut result = Vec::new();
        for channel in self.channels.iter_by_last_packet_sent() {
            if channel.last_packet_sent() >= cutoff {
                break;
            }
            result.push(channel.channel.clone());
        }

        result
    }

    pub fn update(&mut self, endpoint: &SocketAddr) {
        self.channels
            .set_last_packet_sent(endpoint, SystemTime::now());
    }

    pub fn set_last_packet_sent(&mut self, endpoint: &SocketAddr, time: SystemTime) {
        self.channels.set_last_packet_sent(endpoint, time);
    }

    pub fn find_channel(&self, endpoint: &SocketAddr) -> Option<Arc<ChannelEnum>> {
        self.channels.get(endpoint).map(|c| c.channel.clone())
    }

    pub fn random_channels(
        &self,
        count: usize,
        min_version: u8,
        include_temporary_channels: bool,
    ) -> Vec<Arc<ChannelEnum>> {
        let mut result = Vec::with_capacity(count);
        let mut channel_ids = HashSet::new();

        // Stop trying to fill result with random samples after this many attempts
        let random_cutoff = count * 2;
        let peers_size = self.channels.len();
        // Usually count will be much smaller than peers_size
        // Otherwise make sure we have a cutoff on attempting to randomly fill
        if peers_size > 0 {
            let mut rng = thread_rng();
            for _ in 0..random_cutoff {
                let index = rng.gen_range(0..peers_size);
                let wrapper = self.channels.get_by_index(index).unwrap();
                if !wrapper.channel.as_channel().is_alive() {
                    continue;
                }

                if wrapper.tcp_channel().network_version() >= min_version
                    && (include_temporary_channels || !wrapper.channel.as_channel().is_temporary())
                {
                    if channel_ids.insert(wrapper.channel.as_channel().channel_id()) {
                        result.push(wrapper.channel.clone())
                    }
                }

                if result.len() == count {
                    break;
                }
            }
        }

        result
    }
}

#[derive(Default)]
pub struct ChannelContainer {
    by_endpoint: HashMap<SocketAddr, Arc<ChannelTcpWrapper>>,
    by_random_access: Vec<SocketAddr>,
    by_bootstrap_attempt: BTreeMap<SystemTime, Vec<SocketAddr>>,
    by_node_id: HashMap<PublicKey, Vec<SocketAddr>>,
    by_last_packet_sent: BTreeMap<SystemTime, Vec<SocketAddr>>,
    by_network_version: BTreeMap<u8, Vec<SocketAddr>>,
    by_ip_address: HashMap<Ipv6Addr, Vec<SocketAddr>>,
    by_subnet: HashMap<Ipv6Addr, Vec<SocketAddr>>,
}

impl ChannelContainer {
    pub fn insert(&mut self, wrapper: Arc<ChannelTcpWrapper>) -> bool {
        let endpoint = wrapper.endpoint();
        if self.by_endpoint.contains_key(&endpoint) {
            return false;
        }

        self.by_random_access.push(endpoint);
        self.by_bootstrap_attempt
            .entry(wrapper.last_bootstrap_attempt())
            .or_default()
            .push(endpoint);
        self.by_node_id
            .entry(wrapper.node_id().unwrap_or_default())
            .or_default()
            .push(endpoint);
        self.by_last_packet_sent
            .entry(wrapper.last_packet_sent())
            .or_default()
            .push(endpoint);
        self.by_network_version
            .entry(wrapper.network_version())
            .or_default()
            .push(endpoint);
        self.by_ip_address
            .entry(wrapper.ip_address())
            .or_default()
            .push(endpoint);
        self.by_subnet
            .entry(wrapper.subnetwork())
            .or_default()
            .push(endpoint);
        self.by_endpoint.insert(wrapper.endpoint(), wrapper);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<ChannelTcpWrapper>> {
        self.by_endpoint.values()
    }

    pub fn iter_by_last_bootstrap_attempt(&self) -> impl Iterator<Item = &Arc<ChannelTcpWrapper>> {
        self.by_bootstrap_attempt
            .iter()
            .flat_map(|(_, v)| v.iter().map(|ep| self.by_endpoint.get(ep).unwrap()))
    }

    pub fn iter_by_last_packet_sent(&self) -> impl Iterator<Item = &Arc<ChannelTcpWrapper>> {
        self.by_last_packet_sent
            .iter()
            .flat_map(|(_, v)| v.iter().map(|ep| self.by_endpoint.get(ep).unwrap()))
    }

    pub fn exists(&self, endpoint: &SocketAddr) -> bool {
        self.by_endpoint.contains_key(endpoint)
    }

    pub fn remove_by_node_id(&mut self, node_id: &PublicKey) {
        if let Some(endpoints) = self.by_node_id.get(node_id).cloned() {
            for ep in endpoints {
                self.remove_by_endpoint(&ep);
            }
        }
    }

    pub fn remove_by_endpoint(&mut self, endpoint: &SocketAddr) {
        if let Some(wrapper) = self.by_endpoint.remove(endpoint) {
            self.by_random_access.retain(|x| x != endpoint); // todo: linear search is slow?

            remove_endpoint_btree(
                &mut self.by_bootstrap_attempt,
                &wrapper.last_bootstrap_attempt(),
                endpoint,
            );
            remove_endpoint_map(
                &mut self.by_node_id,
                &wrapper.node_id().unwrap_or_default(),
                endpoint,
            );
            remove_endpoint_btree(
                &mut self.by_last_packet_sent,
                &wrapper.last_packet_sent(),
                endpoint,
            );
            remove_endpoint_btree(
                &mut self.by_network_version,
                &wrapper.network_version(),
                endpoint,
            );
            remove_endpoint_map(&mut self.by_ip_address, &wrapper.ip_address(), endpoint);
            remove_endpoint_map(&mut self.by_subnet, &wrapper.subnetwork(), endpoint);
        }
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }

    pub fn get(&self, endpoint: &SocketAddr) -> Option<&Arc<ChannelTcpWrapper>> {
        self.by_endpoint.get(endpoint)
    }

    pub fn get_by_index(&self, index: usize) -> Option<&Arc<ChannelTcpWrapper>> {
        self.by_random_access
            .get(index)
            .map(|ep| self.by_endpoint.get(ep))
            .flatten()
    }

    pub fn get_by_node_id(&self, node_id: &PublicKey) -> Option<&Arc<ChannelTcpWrapper>> {
        self.by_node_id
            .get(node_id)
            .map(|endpoints| self.by_endpoint.get(&endpoints[0]))
            .flatten()
    }

    pub fn set_last_packet_sent(&mut self, endpoint: &SocketAddr, time: SystemTime) {
        if let Some(channel) = self.by_endpoint.get(endpoint) {
            let old_time = channel.last_packet_sent();
            channel.channel.as_channel().set_last_packet_sent(time);
            remove_endpoint_btree(&mut self.by_last_packet_sent, &old_time, endpoint);
            self.by_last_packet_sent
                .entry(time)
                .or_default()
                .push(*endpoint);
        }
    }

    pub fn set_last_bootstrap_attempt(&mut self, endpoint: &SocketAddr, attempt_time: SystemTime) {
        if let Some(channel) = self.by_endpoint.get(endpoint) {
            let old_time = channel.last_bootstrap_attempt();
            channel
                .channel
                .as_channel()
                .set_last_bootstrap_attempt(attempt_time);
            remove_endpoint_btree(
                &mut self.by_bootstrap_attempt,
                &old_time,
                &channel.endpoint(),
            );
            self.by_bootstrap_attempt
                .entry(attempt_time)
                .or_default()
                .push(*endpoint);
        }
    }

    pub fn count_by_ip(&self, ip: &Ipv6Addr) -> usize {
        self.by_ip_address
            .get(ip)
            .map(|endpoints| endpoints.len())
            .unwrap_or_default()
    }

    pub fn count_by_subnet(&self, subnet: &Ipv6Addr) -> usize {
        self.by_subnet
            .get(subnet)
            .map(|endpoints| endpoints.len())
            .unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.by_endpoint.clear();
        self.by_random_access.clear();
        self.by_bootstrap_attempt.clear();
        self.by_node_id.clear();
        self.by_last_packet_sent.clear();
        self.by_network_version.clear();
        self.by_ip_address.clear();
        self.by_subnet.clear();
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        while let Some((time, endpoints)) = self.by_last_packet_sent.first_key_value() {
            if *time < cutoff {
                let endpoints = endpoints.clone();
                for ep in endpoints {
                    self.remove_by_endpoint(&ep);
                }
            } else {
                break;
            }
        }
    }

    pub fn remove_dead(&mut self) {
        let dead_channels: Vec<_> = self
            .by_endpoint
            .values()
            .filter(|c| !c.channel.as_channel().is_alive())
            .cloned()
            .collect();

        for channel in dead_channels {
            self.remove_by_endpoint(&channel.endpoint());
        }
    }

    pub fn remove_old_protocol_versions(&mut self, min_version: u8) {
        while let Some((version, endpoints)) = self.by_network_version.first_key_value() {
            if *version < min_version {
                let endpoints = endpoints.clone();
                for ep in endpoints {
                    self.remove_by_endpoint(&ep);
                }
            } else {
                break;
            }
        }
    }
}

fn remove_endpoint_btree<K: Ord>(
    tree: &mut BTreeMap<K, Vec<SocketAddr>>,
    key: &K,
    endpoint: &SocketAddr,
) {
    let endpoints = tree.get_mut(key).unwrap();
    if endpoints.len() > 1 {
        endpoints.retain(|x| x != endpoint);
    } else {
        tree.remove(key);
    }
}

fn remove_endpoint_map<K: Eq + PartialEq + Hash>(
    map: &mut HashMap<K, Vec<SocketAddr>>,
    key: &K,
    endpoint: &SocketAddr,
) {
    let endpoints = map.get_mut(key).unwrap();
    if endpoints.len() > 1 {
        endpoints.retain(|x| x != endpoint);
    } else {
        map.remove(key);
    }
}

pub struct TcpEndpointAttempt {
    pub endpoint: SocketAddrV6,
    pub address: Ipv6Addr,
    pub subnetwork: Ipv6Addr,
    pub last_attempt: SystemTime,
}

impl TcpEndpointAttempt {
    pub fn new(endpoint: SocketAddrV6) -> Self {
        Self {
            endpoint,
            address: ipv4_address_or_ipv6_subnet(endpoint.ip()),
            subnetwork: map_address_to_subnetwork(endpoint.ip()),
            last_attempt: SystemTime::now(),
        }
    }
}

#[derive(Default)]
pub struct TcpEndpointAttemptContainer {
    by_endpoint: HashMap<SocketAddrV6, TcpEndpointAttempt>,
    by_address: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
    by_subnetwork: HashMap<Ipv6Addr, Vec<SocketAddrV6>>,
    by_time: BTreeMap<SystemTime, Vec<SocketAddrV6>>,
}

impl TcpEndpointAttemptContainer {
    pub fn insert(&mut self, attempt: TcpEndpointAttempt) -> bool {
        if self.by_endpoint.contains_key(&attempt.endpoint) {
            return false;
        }
        self.by_address
            .entry(attempt.address)
            .or_default()
            .push(attempt.endpoint);
        self.by_subnetwork
            .entry(attempt.subnetwork)
            .or_default()
            .push(attempt.endpoint);
        self.by_time
            .entry(attempt.last_attempt)
            .or_default()
            .push(attempt.endpoint);
        self.by_endpoint.insert(attempt.endpoint, attempt);
        true
    }

    pub fn remove(&mut self, endpoint: &SocketAddrV6) {
        if let Some(attempt) = self.by_endpoint.remove(endpoint) {
            let by_address = self.by_address.get_mut(&attempt.address).unwrap();
            if by_address.len() > 1 {
                by_address.retain(|x| x != endpoint);
            } else {
                self.by_address.remove(&attempt.address);
            }

            let by_subnet = self.by_subnetwork.get_mut(&attempt.subnetwork).unwrap();
            if by_subnet.len() > 1 {
                by_subnet.retain(|x| x != endpoint);
            } else {
                self.by_subnetwork.remove(&attempt.subnetwork);
            }

            let by_time = self.by_time.get_mut(&attempt.last_attempt).unwrap();
            if by_time.len() > 1 {
                by_time.retain(|x| x != endpoint);
            } else {
                self.by_time.remove(&attempt.last_attempt);
            }
        }
    }

    pub fn count_by_subnetwork(&self, subnet: &Ipv6Addr) -> usize {
        match self.by_subnetwork.get(subnet) {
            Some(entries) => entries.len(),
            None => 0,
        }
    }

    pub fn count_by_address(&self, address: &Ipv6Addr) -> usize {
        match self.by_address.get(address) {
            Some(entries) => entries.len(),
            None => 0,
        }
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }

    pub fn purge(&mut self, cutoff: SystemTime) {
        while let Some((time, endpoint)) = self.get_oldest() {
            if time >= cutoff {
                return;
            }

            self.remove(&endpoint);
        }
    }

    fn get_oldest(&self) -> Option<(SystemTime, SocketAddrV6)> {
        let (time, endpoints) = self.by_time.first_key_value()?;
        Some((*time, endpoints[0]))
    }
}
