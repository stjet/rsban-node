#include "nano/node/transport/traffic_type.hpp"

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
#include <utility>

/*
 * socket
 */

nano::transport::socket::socket (rsnano::async_runtime & async_rt_a, nano::transport::socket_endpoint endpoint_type_a, nano::stats & stats_a,
std::shared_ptr<nano::thread_pool> const & workers_a,
std::chrono::seconds default_timeout_a, std::chrono::seconds silent_connection_tolerance_time_a,
std::chrono::seconds idle_timeout_a,
std::shared_ptr<nano::node_observers> observers_a,
std::size_t max_queue_size_a)
{
	handle = rsnano::rsn_socket_create (
	static_cast<uint8_t> (endpoint_type_a),
	stats_a.handle,
	workers_a->handle,
	default_timeout_a.count (),
	silent_connection_tolerance_time_a.count (),
	idle_timeout_a.count (),
	new std::weak_ptr<nano::node_observers> (observers_a),
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

void async_connect_adapter (void * context, rsnano::ErrorCodeDto const * error)
{
	try
	{
		auto ec{ rsnano::dto_to_error_code (*error) };
		auto callback = static_cast<std::function<void (boost::system::error_code const &)> *> (context);
		(*callback) (ec);
	}
	catch (...)
	{
		std::cerr << "exception in async_connect_adapter!" << std::endl;
	}
}

void async_connect_delete_context (void * context)
{
	auto callback = static_cast<std::function<void (boost::system::error_code const &)> *> (context);
	delete callback;
}

boost::asio::ip::tcp::endpoint & nano::transport::socket::get_remote ()
{
	return remote;
}

void nano::transport::socket::async_connect (nano::tcp_endpoint const & endpoint_a, std::function<void (boost::system::error_code const &)> callback_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	auto cb_wrapper = new std::function<void (boost::system::error_code const &)> ([callback = std::move (callback_a), this_l = shared_from_this ()] (boost::system::error_code const & ec) {
		callback (ec);
	});
	rsnano::rsn_socket_async_connect (handle, &endpoint_dto, async_connect_adapter, async_connect_delete_context, cb_wrapper);
}

void nano::transport::async_read_adapter (void * context_a, rsnano::ErrorCodeDto const * error_a, std::size_t size_a)
{
	try
	{
		auto ec{ rsnano::dto_to_error_code (*error_a) };
		auto callback = static_cast<std::function<void (boost::system::error_code const &, std::size_t)> *> (context_a);
		(*callback) (ec, size_a);
	}
	catch (...)
	{
		std::cerr << "exception in async_read_adapter!" << std::endl;
	}
}

void nano::transport::async_read_delete_context (void * context_a)
{
	auto callback = static_cast<std::function<void (boost::system::error_code const &, std::size_t)> *> (context_a);
	delete callback;
}

void nano::transport::socket::async_write (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> callback_a, nano::transport::traffic_type traffic_type)
{
	auto cb_wrapper = new std::function<void (boost::system::error_code const &, std::size_t)> ([callback = std::move (callback_a), this_l = shared_from_this ()] (boost::system::error_code const & ec, std::size_t size) {
		callback (ec, size);
	});

	auto buffer_l = buffer_a.to_bytes ();
	rsnano::rsn_socket_async_write (handle, buffer_l.data (), buffer_l.size (), async_read_adapter, async_read_delete_context, cb_wrapper, static_cast<uint8_t> (traffic_type));
}

void nano::transport::socket::close_internal ()
{
	rsnano::rsn_socket_close_internal (handle);
}

void nano::transport::socket::checkup ()
{
	rsnano::rsn_socket_checkup (handle);
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
	node_a.observers,
	write_queue_size);
}
