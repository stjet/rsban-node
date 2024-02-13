use tracing::debug;

use super::{Socket, SocketExtensions};
use crate::utils::{first_ipv6_subnet_address, last_ipv6_subnet_address};
use std::{
    collections::BTreeMap,
    net::Ipv6Addr,
    sync::{Arc, Weak},
};

#[derive(Default)]
pub(crate) struct ConnectionsPerAddress {
    connections: BTreeMap<Ipv6Addr, Vec<Weak<Socket>>>,
    count: usize,
}
impl ConnectionsPerAddress {
    pub(crate) fn close_connections(&mut self) {
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

    pub(crate) fn count_subnetwork_connections(
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

    pub(crate) fn count_connections_for_ip(&self, ip: &Ipv6Addr) -> usize {
        self.connections
            .get(ip)
            .map(|conns| conns.len())
            .unwrap_or_default()
    }

    pub(crate) fn count_connections(&self) -> usize {
        self.count
    }

    pub(crate) fn insert(&mut self, connection: &Arc<Socket>) {
        debug!(
            socket_id = connection.socket_id,
            "Inserting into ConnectionsPerAddress"
        );
        let endpoint = connection.get_remote().unwrap();
        self.connections
            .entry(*endpoint.ip())
            .or_default()
            .push(Arc::downgrade(connection));
        self.count += 1;
    }

    pub(crate) fn evict_dead_connections(&mut self) {
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
    use std::net::SocketAddrV6;

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
            socket.set_remote(SocketAddrV6::new(ip, 42, 0, 0));
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
