use std::{
    collections::{BTreeMap, HashMap},
    net::{Ipv6Addr, SocketAddrV6},
    time::SystemTime,
};

use crate::utils::{ipv4_address_or_ipv6_subnet, map_address_to_subnetwork};

pub struct TcpChannels {
    pub attempts: TcpEndpointAttemptContainer,
}

impl TcpChannels {
    pub fn new() -> Self {
        Self {
            attempts: Default::default(),
        }
    }
}

impl Default for TcpChannels {
    fn default() -> Self {
        Self::new()
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
