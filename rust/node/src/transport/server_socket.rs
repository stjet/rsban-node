use std::{
    collections::BTreeMap,
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex, Weak},
};

use crate::utils::{last_ipv6_subnet_address, make_network_address};

use super::{Socket, SocketExtensions, TcpSocketFacade};

pub struct ServerSocket {
    socket_facade: Arc<dyn TcpSocketFacade>,
    connections_per_address: Mutex<BTreeMap<Ipv6Addr, Vec<Weak<Socket>>>>,
}

impl ServerSocket {
    pub fn new(socket_facade: Arc<dyn TcpSocketFacade>) -> Self {
        ServerSocket {
            socket_facade,
            connections_per_address: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn close_connections(&self) {
        let mut guard = self.connections_per_address.lock().unwrap();
        for conns in guard.values() {
            for conn in conns.iter() {
                if let Some(conn) = conn.upgrade() {
                    conn.close();
                }
            }
        }
        guard.clear();
    }

    pub fn count_subnetwork_connections(
        &self,
        remote_address: &SocketAddrV6,
        network_prefix: usize,
    ) -> usize {
        let first_ip = make_network_address(remote_address.ip(), network_prefix as u8);
        let last_ip = last_ipv6_subnet_address(remote_address.ip(), network_prefix as u8);
        let guard = self.connections_per_address.lock().unwrap();
        guard
            .range(first_ip..=last_ip)
            .map(|(_, conns)| conns.len())
            .sum()
    }

    pub fn count_connections_for_ip(&self, ip: &Ipv6Addr) -> usize {
        let guard = self.connections_per_address.lock().unwrap();
        guard.get(ip).map(|conns| conns.len()).unwrap_or_default()
    }

    pub fn count_connections(&self) -> usize {
        let guard = self.connections_per_address.lock().unwrap();
        //todo optimize!
        guard.values().map(|conns| conns.len()).sum()
    }

    pub fn insert_connection(&self, connection: &Arc<Socket>) {
        let mut guard = self
            .connections_per_address
            .lock()
            .expect("socket doesn't have a remote endpoint'");
        let SocketAddr::V6(endpoint) = connection.get_remote().unwrap() else { panic!("socket doesn't have a v6 remote endpoint'");};
        guard
            .entry(*endpoint.ip())
            .or_default()
            .push(Arc::downgrade(connection));
    }

    pub fn evict_dead_connections(&self) {
        let mut guard = self.connections_per_address.lock().unwrap();
        guard.retain(|_, conns| {
            conns.retain(|c| c.strong_count() > 0);
            !conns.is_empty()
        });
    }
}
