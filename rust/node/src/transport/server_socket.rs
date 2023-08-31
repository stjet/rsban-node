use std::{
    collections::BTreeMap,
    net::{Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex, Weak},
};

use crate::utils::{first_ipv6_subnet_address, last_ipv6_subnet_address};

use super::{Socket, SocketExtensions, TcpSocketFacade};

pub struct ServerSocket {
    socket_facade: Arc<dyn TcpSocketFacade>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
}

impl ServerSocket {
    pub fn new(socket_facade: Arc<dyn TcpSocketFacade>) -> Self {
        ServerSocket {
            socket_facade,
            connections_per_address: Mutex::new(Default::default()),
        }
    }

    pub fn close_connections(&self) {
        self.connections_per_address
            .lock()
            .unwrap()
            .close_connections();
    }

    pub fn count_subnetwork_connections(
        &self,
        remote_address: &Ipv6Addr,
        network_prefix: usize,
    ) -> usize {
        self.connections_per_address
            .lock()
            .unwrap()
            .count_subnetwork_connections(remote_address, network_prefix)
    }

    pub fn count_connections_for_ip(&self, ip: &Ipv6Addr) -> usize {
        self.connections_per_address
            .lock()
            .unwrap()
            .count_connections_for_ip(ip)
    }

    pub fn count_connections(&self) -> usize {
        self.connections_per_address
            .lock()
            .unwrap()
            .count_connections()
    }

    pub fn insert_connection(&self, connection: &Arc<Socket>) {
        self.connections_per_address
            .lock()
            .unwrap()
            .insert(connection);
    }

    pub fn evict_dead_connections(&self) {
        self.connections_per_address
            .lock()
            .unwrap()
            .evict_dead_connections();
    }
}

#[derive(Default)]
struct ConnectionsPerAddress {
    connections: BTreeMap<Ipv6Addr, Vec<Weak<Socket>>>,
    count: usize,
}
impl ConnectionsPerAddress {
    pub fn close_connections(&mut self) {
        for conns in self.connections.values() {
            for conn in conns.iter() {
                if let Some(conn) = conn.upgrade() {
                    conn.close();
                }
            }
        }
        self.connections.clear();
        self.count = 0;
    }

    pub fn count_subnetwork_connections(
        &self,
        remote_address: &Ipv6Addr,
        network_prefix: usize,
    ) -> usize {
        let first_ip = first_ipv6_subnet_address(remote_address, network_prefix as u8);
        let last_ip = last_ipv6_subnet_address(remote_address, network_prefix as u8);
        self.connections
            .range(first_ip..=last_ip)
            .map(|(_, conns)| conns.len())
            .sum()
    }

    pub fn count_connections_for_ip(&self, ip: &Ipv6Addr) -> usize {
        self.connections
            .get(ip)
            .map(|conns| conns.len())
            .unwrap_or_default()
    }

    pub fn count_connections(&self) -> usize {
        self.count
    }

    pub fn insert(&mut self, connection: &Arc<Socket>) {
        let SocketAddr::V6(endpoint) = connection.get_remote().unwrap() else { panic!("socket doesn't have a v6 remote endpoint'");};
        self.connections
            .entry(*endpoint.ip())
            .or_default()
            .push(Arc::downgrade(connection));
        self.count += 1;
    }

    pub fn evict_dead_connections(&mut self) {
        self.connections.retain(|_, conns| {
            conns.retain(|c| c.strong_count() > 0);
            !conns.is_empty()
        });
        self.count = self.connections.values().map(|conns| conns.len()).sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn count_subnetwork_connections() {
        let addresses = [
            // out of network prefix
            Ipv6Addr::new(
                0xa41d, 0xb7b1, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
            ),
            // reference address
            Ipv6Addr::new(
                0xa41d, 0xb7b2, 0x8298, 0xcf45, 0x672e, 0xbd1a, 0xe7fb, 0xf713,
            ),
            // start of the network range
            Ipv6Addr::new(0xa41d, 0xb7b2, 0, 0, 0, 0, 0, 0),
            Ipv6Addr::new(0xa41d, 0xb7b2, 0, 0, 0, 0, 0, 1),
            // end of the network range
            Ipv6Addr::new(
                0xa41d, 0xb7b2, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
            ),
            // out of the network prefix
            Ipv6Addr::new(0xa41d, 0xb7b3, 0, 0, 0, 0, 0, 0),
            Ipv6Addr::new(0xa41d, 0xb7b3, 0, 0, 0, 0, 0, 1),
        ];

        let mut connections = ConnectionsPerAddress::default();
        for ip in addresses {
            let socket = Socket::create_null();
            socket.set_remote(SocketAddr::new(IpAddr::V6(ip), 42));
            connections.insert(&socket);
        }

        // Asserts it counts only the connections for the specified address and its network prefix.
        let count = connections.count_subnetwork_connections(
            &Ipv6Addr::new(
                0xa41d, 0xb7b2, 0x8298, 0xcf45, 0x672e, 0xbd1a, 0xe7fb, 0xf713,
            ),
            32,
        );
        assert_eq!(count, 4);
    }
}
