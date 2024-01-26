use super::{Socket, SocketExtensions, SocketObserver, TcpSocketFacadeFactory, TokioSocketFacade};
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::Stats,
    utils::{
        first_ipv6_subnet_address, is_ipv4_mapped, last_ipv6_subnet_address, AsyncRuntime,
        ErrorCode, ThreadPool,
    },
    NetworkParams,
};
use rsnano_core::utils::Logger;
use std::{
    collections::BTreeMap,
    net::Ipv6Addr,
    sync::{Arc, Mutex, Weak},
};

pub struct ServerSocket {
    pub socket: Arc<Socket>,
    pub socket_facade: Arc<TokioSocketFacade>,
    pub connections_per_address: Mutex<ConnectionsPerAddress>,
    pub node_flags: NodeFlags,
    pub network_params: NetworkParams,
    pub workers: Arc<dyn ThreadPool>,
    pub logger: Arc<dyn Logger>,
    pub tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    pub node_config: NodeConfig,
    pub stats: Arc<Stats>,
    pub socket_observer: Weak<dyn SocketObserver>,
    pub max_inbound_connections: usize,
    pub runtime: Weak<AsyncRuntime>,
}

impl ServerSocket {
    pub fn limit_reached_for_incoming_ip_connections(&self, new_connection: &Arc<Socket>) -> bool {
        if self.node_flags.disable_max_peers_per_ip {
            return false;
        }

        let ip = new_connection.get_remote().unwrap().ip().clone();
        let counted_connections = self
            .connections_per_address
            .lock()
            .unwrap()
            .count_connections_for_ip(&ip);

        counted_connections >= self.network_params.network.max_peers_per_ip
    }

    pub fn limit_reached_for_incoming_subnetwork_connections(
        &self,
        new_connection: &Arc<Socket>,
    ) -> bool {
        let endpoint = new_connection
            .get_remote()
            .expect("new connection has no remote endpoint set");
        if self.node_flags.disable_max_peers_per_subnetwork || is_ipv4_mapped(&endpoint.ip()) {
            // If the limit is disabled, then it is unreachable.
            // If the address is IPv4 we don't check for a network limit, since its address space isn't big as IPv6 /64.
            return false;
        }

        let counted_connections = self
            .connections_per_address
            .lock()
            .unwrap()
            .count_subnetwork_connections(
                endpoint.ip(),
                self.network_params
                    .network
                    .ipv6_subnetwork_prefix_for_limiting,
            );

        counted_connections >= self.network_params.network.max_peers_per_subnetwork
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
pub struct ConnectionsPerAddress {
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
        let endpoint = connection.get_remote().unwrap();
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

pub trait ServerSocketExtensions {
    /// Stop accepting new connections
    fn close(&self);
}

impl ServerSocketExtensions for Arc<ServerSocket> {
    fn close(&self) {
        let self_clone = Arc::clone(self);
        //self.socket_facade.dispatch(Box::new(move || {
        self_clone.socket.close_internal();
        self_clone.socket_facade.close_acceptor();
        self_clone
            .connections_per_address
            .lock()
            .unwrap()
            .close_connections();
        //}))
    }
}

fn is_temporary_error(ec: ErrorCode) -> bool {
    return ec.val == 11 // would block
                        || ec.val ==  4; // interrupted system call
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
