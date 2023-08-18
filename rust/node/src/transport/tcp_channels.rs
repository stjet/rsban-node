use std::{
    collections::{BTreeMap, HashMap},
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::Arc,
    time::SystemTime,
};

use rsnano_core::PublicKey;

use crate::{
    bootstrap::ChannelTcpWrapper,
    utils::{ipv4_address_or_ipv6_subnet, map_address_to_subnetwork},
};

pub struct TcpChannels {
    pub attempts: TcpEndpointAttemptContainer,
    pub channels: ChannelContainer,
}

impl TcpChannels {
    pub fn new() -> Self {
        Self {
            attempts: Default::default(),
            channels: Default::default(),
        }
    }
}

impl Default for TcpChannels {
    fn default() -> Self {
        Self::new()
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

            let by_bootstrap = self
                .by_bootstrap_attempt
                .get_mut(&wrapper.last_bootstrap_attempt())
                .unwrap();
            if by_bootstrap.len() > 1 {
                by_bootstrap.retain(|x| x != endpoint);
            } else {
                self.by_bootstrap_attempt
                    .remove(&wrapper.last_bootstrap_attempt());
            }

            // by_node_id: HashMap<PublicKey, Vec<SocketAddr>>,
            // by_last_packet_sent: BTreeMap<SystemTime, Vec<SocketAddr>>,
            // by_network_version: BTreeMap<u8, Vec<SocketAddr>>,
            // by_ip_address: HashMap<Ipv6Addr, Vec<SocketAddr>>,
            // by_subnet: HashMap<Ipv6Addr, Vec<SocketAddr>>,
        }
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
