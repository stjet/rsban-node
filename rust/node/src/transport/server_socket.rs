use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use rsnano_core::utils::Logger;

use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, SocketStats, StatType, Stats},
    utils::{
        first_ipv6_subnet_address, into_ipv6_address, is_ipv4_or_v4_mapped_address,
        last_ipv6_subnet_address, ErrorCode, ThreadPool,
    },
    NetworkParams,
};

use super::{
    CompositeSocketObserver, EndpointType, Socket, SocketBuilder, SocketExtensions, SocketObserver,
    TcpSocketFacade, TcpSocketFacadeFactory,
};

pub struct ServerSocket {
    socket: Arc<Socket>,
    socket_facade: Arc<dyn TcpSocketFacade>,
    connections_per_address: Mutex<ConnectionsPerAddress>,
    node_flags: NodeFlags,
    network_params: NetworkParams,
    workers: Arc<dyn ThreadPool>,
    logger: Arc<dyn Logger>,
    tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
    node_config: NodeConfig,
    stats: Arc<Stats>,
    socket_observer: Arc<dyn SocketObserver>,
    max_inbound_connections: usize,
    local: SocketAddr,
}

impl ServerSocket {
    pub fn new(
        socket_facade: Arc<dyn TcpSocketFacade>,
        node_flags: NodeFlags,
        network_params: NetworkParams,
        workers: Arc<dyn ThreadPool>,
        logger: Arc<dyn Logger>,
        tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
        node_config: NodeConfig,
        stats: Arc<Stats>,
        socket_observer: Arc<dyn SocketObserver>,
        max_inbound_connections: usize,
        local: SocketAddr,
    ) -> Self {
        let socket_stats = Arc::new(SocketStats::new(
            Arc::clone(&stats),
            Arc::clone(&logger),
            node_config.logging.network_timeout_logging(),
        ));
        let ffi_observer = Arc::clone(&socket_observer);

        let socket = SocketBuilder::endpoint_type(
            EndpointType::Server,
            Arc::clone(&socket_facade),
            Arc::clone(&workers),
        )
        .default_timeout(Duration::MAX)
        .silent_connection_tolerance_time(Duration::from_secs(
            network_params.network.silent_connection_tolerance_time_s as u64,
        ))
        .idle_timeout(Duration::from_secs(
            network_params.network.idle_timeout_s as u64,
        ))
        .observer(Arc::new(CompositeSocketObserver::new(vec![
            socket_stats,
            ffi_observer,
        ])))
        .build();

        ServerSocket {
            socket,
            socket_facade,
            connections_per_address: Mutex::new(Default::default()),
            node_flags,
            network_params,
            workers,
            logger,
            tcp_socket_facade_factory,
            node_config,
            stats,
            socket_observer,
            max_inbound_connections,
            local,
        }
    }

    pub fn limit_reached_for_incoming_ip_connections(&self, new_connection: &Arc<Socket>) -> bool {
        if self.node_flags.disable_max_peers_per_ip {
            return false;
        }

        let ip = into_ipv6_address(new_connection.get_remote().unwrap().ip());
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
        if self.node_flags.disable_max_peers_per_subnetwork
            || is_ipv4_or_v4_mapped_address(&endpoint.ip())
        {
            // If the limit is disabled, then it is unreachable.
            // If the address is IPv4 we don't check for a network limit, since its address space isn't big as IPv6 /64.
            return false;
        }
        let ip_address = into_ipv6_address(endpoint.ip());

        let counted_connections = self
            .connections_per_address
            .lock()
            .unwrap()
            .count_subnetwork_connections(
                &ip_address,
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

    pub fn start(&self) -> ErrorCode {
        self.socket_facade.open(&self.local)
    }

    pub fn listening_port(&self) -> u16 {
        self.socket_facade.listening_port()
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
        let SocketAddr::V6(endpoint) = connection.get_remote().unwrap() else {
            panic!("socket doesn't have a v6 remote endpoint'");
        };
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
    fn on_connection(&self, callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>);
    /// Stop accepting new connections
    fn close(&self);

    /// If we are unable to accept a socket, for any reason, we wait just a little (1ms) before rescheduling the next connection accept.
    /// The intention is to throttle back the connection requests and break up any busy loops that could possibly form and
    /// give the rest of the system a chance to recover.
    fn on_connection_requeue_delayed(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    );
}

impl ServerSocketExtensions for Arc<ServerSocket> {
    fn close(&self) {
        let self_clone = Arc::clone(self);
        self.socket_facade.dispatch(Box::new(move || {
            self_clone.socket.close_internal();
            self_clone.socket_facade.close_acceptor();
            self_clone
                .connections_per_address
                .lock()
                .unwrap()
                .close_connections();
        }))
    }

    fn on_connection(&self, callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>) {
        let this_l = Arc::clone(self);
        self.socket_facade.post(Box::new(move || {
            if !this_l.socket_facade.is_acceptor_open() {
                this_l.logger.always_log("Network: Acceptor is not open");
                return;
            }

            let socket_stats = Arc::new(SocketStats::new(
                Arc::clone(&this_l.stats),
                Arc::clone(&this_l.logger),
                this_l.node_config.logging.network_timeout_logging(),
            ));
            let ffi_observer = Arc::clone(&this_l.socket_observer);

            let client_socket = this_l.tcp_socket_facade_factory.create_tcp_socket();
            // Prepare new connection
            let new_connection = SocketBuilder::endpoint_type(
                EndpointType::Server,
                Arc::clone(&client_socket),
                Arc::clone(&this_l.workers),
            )
            .default_timeout(Duration::from_secs(
                this_l.node_config.tcp_io_timeout_s as u64,
            ))
            .idle_timeout(Duration::from_secs(
                this_l
                    .network_params
                    .network
                    .silent_connection_tolerance_time_s as u64,
            ))
            .silent_connection_tolerance_time(Duration::from_secs(
                this_l
                    .network_params
                    .network
                    .silent_connection_tolerance_time_s as u64,
            ))
            .observer(Arc::new(CompositeSocketObserver::new(vec![
                socket_stats,
                ffi_observer,
            ])))
            .build();

            let this_clone = Arc::clone(&this_l);
            this_l.socket_facade.async_accept(
                &client_socket,
                Box::new(move |remote_endpoint, ec| {
                    let this_l = this_clone;
                    new_connection.set_remote(remote_endpoint);
                    this_l.evict_dead_connections();

                    if this_l.connections_per_address.lock().unwrap().count_connections() >= this_l.max_inbound_connections {
                        this_l.logger.try_log ("Network: max_inbound_connections reached, unable to open new connection");
                        this_l.stats.inc (StatType::Tcp, DetailType::TcpAcceptFailure, Direction::In);
                        this_l.on_connection_requeue_delayed (callback);
                        return;
                    }

                    if this_l.limit_reached_for_incoming_ip_connections (&new_connection) {
                        let remote_ip_address = new_connection.get_remote().unwrap().ip();
                        let log_message = format!("Network: max connections per IP (max_peers_per_ip) was reached for {}, unable to open new connection", remote_ip_address);
                        this_l.logger.try_log(&log_message);
                        this_l.stats.inc (StatType::Tcp, DetailType::TcpMaxPerIp, Direction::In);
                        this_l.on_connection_requeue_delayed (callback);
                        return;
                    }

                    if this_l.limit_reached_for_incoming_subnetwork_connections (&new_connection) {
                        let remote_ip_address = new_connection.get_remote().unwrap().ip();
                        let IpAddr::V6(remote_ip_address) = remote_ip_address else { panic!("not a v6 IP address")};
                        let remote_subnet = first_ipv6_subnet_address(&remote_ip_address, this_l.network_params.network.max_peers_per_subnetwork as u8);
                        let log_message = format!("Network: max connections per subnetwork (max_peers_per_subnetwork) was reached for subnetwork {} (remote IP: {}), unable to open new connection",
                            remote_subnet, remote_ip_address);
                        this_l.logger.try_log(&log_message);
                        this_l.stats.inc(StatType::Tcp, DetailType::TcpMaxPerSubnetwork, Direction::In);
                        this_l.on_connection_requeue_delayed (callback);
                        return;
                    }
                   			if ec.is_ok() {
                    				// Make sure the new connection doesn't idle. Note that in most cases, the callback is going to start
                    				// an IO operation immediately, which will start a timer.
                    				new_connection.start ();
                    				new_connection.set_timeout (Duration::from_secs(this_l.network_params.network.idle_timeout_s as u64));
                    				this_l.stats.inc (StatType::Tcp, DetailType::TcpAcceptSuccess, Direction::In);
                                    this_l.connections_per_address.lock().unwrap().insert(&new_connection);
                    				this_l.socket_observer.socket_accepted(Arc::clone(&new_connection));
                    				if callback (new_connection, ec)
                    				{
                    					this_l.on_connection (callback);
                    					return;
                    				}
                    				this_l.logger.always_log ("Network: Stopping to accept connections");
                    				return;
                   			}

                    			// accept error
                    			this_l.logger.try_log (&format!("Network: Unable to accept connection: {:?}", ec));
                    			this_l.stats.inc (StatType::Tcp, DetailType::TcpAcceptFailure, Direction::In);

                    			if is_temporary_error (ec)
                    			{
                    				// if it is a temporary error, just retry it
                    				this_l.on_connection_requeue_delayed (callback);
                    				return;
                    			}

                    			// if it is not a temporary error, check how the listener wants to handle this error
                    			if callback(new_connection, ec)
                    			{
                    				this_l.on_connection_requeue_delayed (callback);
                    				return;
                    			}

                    			// No requeue if we reach here, no incoming socket connections will be handled
                    			this_l.logger.always_log ("Network: Stopping to accept connections");
                }),
            );
        }))
    }

    fn on_connection_requeue_delayed(
        &self,
        callback: Box<dyn Fn(Arc<Socket>, ErrorCode) -> bool + Send + Sync>,
    ) {
        let this_l = Arc::clone(self);
        self.workers.add_delayed_task(
            Duration::from_millis(1),
            Box::new(move || {
                this_l.on_connection(callback);
            }),
        );
    }
}

fn is_temporary_error(ec: ErrorCode) -> bool {
    return ec.val == 11 // would block
                        || ec.val ==  4; // interrupted system call
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
