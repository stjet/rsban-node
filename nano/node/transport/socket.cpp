#include <nano/boost/asio/bind_executor.hpp>
#include <nano/boost/asio/ip/address_v6.hpp>
#include <nano/boost/asio/read.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/socket.hpp>
#include <nano/node/transport/transport.hpp>

#include <boost/format.hpp>

#include <cstdint>
#include <cstdlib>
#include <memory>

/*
 * socket
 */

nano::transport::socket::socket (rsnano::async_runtime & async_rt_a, nano::transport::socket_endpoint endpoint_type_a, nano::stats & stats_a,
std::shared_ptr<nano::thread_pool> const & workers_a,
std::chrono::seconds default_timeout_a, std::chrono::seconds silent_connection_tolerance_time_a,
std::chrono::seconds idle_timeout_a,
std::size_t max_queue_size_a)
{
	handle = rsnano::rsn_socket_create (
	static_cast<uint8_t> (endpoint_type_a),
	stats_a.handle,
	workers_a->handle,
	default_timeout_a.count (),
	silent_connection_tolerance_time_a.count (),
	idle_timeout_a.count (),
	max_queue_size_a,
	async_rt_a.handle);
}

nano::transport::socket::socket (rsnano::SocketHandle * handle_a) :
	handle{ handle_a }
{
}

nano::transport::socket::~socket ()
{
	rsnano::rsn_socket_destroy (handle);
}

boost::asio::ip::network_v6 nano::transport::socket_functions::get_ipv6_subnet_address (boost::asio::ip::address_v6 const & ip_address, std::size_t network_prefix)
{
	return boost::asio::ip::make_network_v6 (ip_address, static_cast<unsigned short> (network_prefix));
}

std::shared_ptr<nano::transport::socket> nano::transport::create_client_socket (nano::node & node_a, std::size_t write_queue_size)
{
	return std::make_shared<nano::transport::socket> (node_a.async_rt, nano::transport::socket_endpoint::client, *node_a.stats, node_a.workers,
	node_a.config->tcp_io_timeout,
	node_a.network_params.network.silent_connection_tolerance_time,
	node_a.network_params.network.idle_timeout,
	write_queue_size);
}
