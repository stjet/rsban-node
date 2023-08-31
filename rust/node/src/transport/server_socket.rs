use std::{
    collections::BTreeMap,
    net::{Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use rsnano_core::utils::Logger;

use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{SocketStats, Stats},
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
}

impl ServerSocket {
    pub fn new(
        socket_facade: Arc<dyn TcpSocketFacade>,
        socket: Arc<Socket>,
        node_flags: NodeFlags,
        network_params: NetworkParams,
        workers: Arc<dyn ThreadPool>,
        logger: Arc<dyn Logger>,
        tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory>,
        node_config: NodeConfig,
        stats: Arc<Stats>,
        socket_observer: Arc<dyn SocketObserver>,
    ) -> Self {
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

pub trait ServerSocketExtensions {
    fn on_connection(&self, callback: Box<dyn Fn(Arc<Socket>, ErrorCode)>);
    /// Stop accepting new connections
    fn close(&self);
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

    fn on_connection(&self, _callback: Box<dyn Fn(Arc<Socket>, ErrorCode)>) {
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

            // Prepare new connection
            let _new_connection = SocketBuilder::endpoint_type(
                EndpointType::Server,
                this_l.tcp_socket_facade_factory.create_tcp_socket(),
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

            //		auto socket_facade_ptr = static_cast<std::shared_ptr<nano::transport::tcp_socket_facade> *> (rsnano::rsn_socket_facade (new_connection->handle));
            //		std::shared_ptr<nano::transport::tcp_socket_facade> client_socket_facade (*socket_facade_ptr);
            //		this_l->socket_facade->async_accept (
            //		client_socket_facade->tcp_socket,
            //		new_connection->get_remote (),
            //		[this_l, new_connection, cbk = std::move (callback)] (boost::system::error_code const & ec_a) mutable {
            //			auto endpoint_dto{ rsnano::endpoint_to_dto (new_connection->get_remote ()) };
            //			rsnano::rsn_socket_set_remote_endpoint (new_connection->handle, &endpoint_dto);
            //			this_l->evict_dead_connections ();
            //
            //			if (rsnano::rsn_server_socket_count_connections (this_l->handle) >= this_l->max_inbound_connections)
            //			{
            //				this_l->logger.try_log ("Network: max_inbound_connections reached, unable to open new connection");
            //				this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_accept_failure, nano::stat::dir::in);
            //				this_l->on_connection_requeue_delayed (std::move (cbk));
            //				return;
            //			}
            //
            //			if (this_l->limit_reached_for_incoming_ip_connections (new_connection))
            //			{
            //				auto const remote_ip_address = new_connection->remote_endpoint ().address ();
            //				auto const log_message = boost::str (
            //				boost::format ("Network: max connections per IP (max_peers_per_ip) was reached for %1%, unable to open new connection")
            //				% remote_ip_address.to_string ());
            //				this_l->logger.try_log (log_message);
            //				this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_max_per_ip, nano::stat::dir::in);
            //				this_l->on_connection_requeue_delayed (std::move (cbk));
            //				return;
            //			}
            //
            //			if (this_l->limit_reached_for_incoming_subnetwork_connections (new_connection))
            //			{
            //				auto const remote_ip_address = new_connection->remote_endpoint ().address ();
            //				debug_assert (remote_ip_address.is_v6 ());
            //				auto const remote_subnet = socket_functions::get_ipv6_subnet_address (remote_ip_address.to_v6 (), this_l->node.network_params.network.max_peers_per_subnetwork);
            //				auto const log_message = boost::str (
            //				boost::format ("Network: max connections per subnetwork (max_peers_per_subnetwork) was reached for subnetwork %1% (remote IP: %2%), unable to open new connection")
            //				% remote_subnet.canonical ().to_string ()
            //				% remote_ip_address.to_string ());
            //				this_l->logger.try_log (log_message);
            //				this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_max_per_subnetwork, nano::stat::dir::in);
            //				this_l->on_connection_requeue_delayed (std::move (cbk));
            //				return;
            //			}
            //
            //			if (!ec_a)
            //			{
            //				// Make sure the new connection doesn't idle. Note that in most cases, the callback is going to start
            //				// an IO operation immediately, which will start a timer.
            //				new_connection->start ();
            //				new_connection->set_timeout (this_l->node.network_params.network.idle_timeout);
            //				this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_accept_success, nano::stat::dir::in);
            //				rsnano::rsn_server_socket_insert_connection (this_l->handle, new_connection->handle);
            //				this_l->node.observers->socket_accepted.notify (*new_connection);
            //				if (cbk (new_connection, ec_a))
            //				{
            //					this_l->on_connection (std::move (cbk));
            //					return;
            //				}
            //				this_l->logger.always_log ("Network: Stopping to accept connections");
            //				return;
            //			}
            //
            //			// accept error
            //			this_l->logger.try_log ("Network: Unable to accept connection: ", ec_a.message ());
            //			this_l->stats.inc (nano::stat::type::tcp, nano::stat::detail::tcp_accept_failure, nano::stat::dir::in);
            //
            //			if (is_temporary_error (ec_a))
            //			{
            //				// if it is a temporary error, just retry it
            //				this_l->on_connection_requeue_delayed (std::move (cbk));
            //				return;
            //			}
            //
            //			// if it is not a temporary error, check how the listener wants to handle this error
            //			if (cbk (new_connection, ec_a))
            //			{
            //				this_l->on_connection_requeue_delayed (std::move (cbk));
            //				return;
            //			}
            //
            //			// No requeue if we reach here, no incoming socket connections will be handled
            //			this_l->logger.always_log ("Network: Stopping to accept connections");
            //		});
        }))
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
