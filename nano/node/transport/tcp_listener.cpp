#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/node/transport/tcp_server.hpp>
/*
 * tcp_listener
 */

nano::transport::tcp_listener::tcp_listener (uint16_t port_a, nano::node & node_a, std::size_t max_inbound_connections)
{
	auto config_dto{ node_a.config->to_dto () };
	auto network_params_dto{ node_a.network_params.to_dto () };

	handle = rsnano::rsn_tcp_listener_create (
	port_a,
	max_inbound_connections,
	&config_dto,
	node_a.network->tcp_channels->handle,
	node_a.network->syn_cookies->handle,
	&network_params_dto,
	node_a.flags.handle,
	node_a.async_rt.handle,
	node_a.stats->handle,
	node_a.bootstrap_workers->handle,
	new std::weak_ptr<nano::node_observers> (node_a.observers),
	node_a.block_processor.handle,
	node_a.bootstrap_initiator.handle,
	node_a.ledger.handle,
	node_a.node_id.prv.bytes.data ());
}

nano::transport::tcp_listener::~tcp_listener ()
{
	rsnano::rsn_tcp_listener_destroy (handle);
}

namespace
{
bool on_connection_callback (void * context, rsnano::SocketHandle * socket_handle, const rsnano::ErrorCodeDto * ec_dto)
{
	auto callback = static_cast<std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> *> (context);
	auto socket = std::make_shared<nano::transport::socket> (socket_handle);
	auto ec = rsnano::dto_to_error_code (*ec_dto);
	return (*callback) (socket, ec);
}

void delete_on_connection_context (void * handle_a)
{
	auto callback = static_cast<std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> *> (handle_a);
	delete callback;
}
}

void nano::transport::tcp_listener::start (std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> callback_a)
{
	auto context = new std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> (callback_a);
	bool ok = rsnano::rsn_tcp_listener_start (handle, on_connection_callback, context, delete_on_connection_context);
	if (!ok)
		throw new std::runtime_error ("could not start tcp listener");
	return;
}

void nano::transport::tcp_listener::stop ()
{
	rsnano::rsn_tcp_listener_stop (handle);
}

std::size_t nano::transport::tcp_listener::connection_count ()
{
	return rsnano::rsn_tcp_listener_connection_count (handle);
}

std::size_t nano::transport::tcp_listener::get_realtime_count ()
{
	return rsnano::rsn_tcp_listener_realtime_count (handle);
}

void nano::transport::tcp_listener::accept_action (boost::system::error_code const & ec, std::shared_ptr<nano::transport::socket> const & socket_a)
{
	auto ec_dto{ rsnano::error_code_to_dto (ec) };
	rsnano::rsn_tcp_listener_accept_action (handle, &ec_dto, socket_a->handle);
}

boost::asio::ip::tcp::endpoint nano::transport::tcp_listener::endpoint ()
{
	rsnano::EndpointDto endpoint_dto{};
	rsnano::rsn_tcp_listener_endpoint (handle, &endpoint_dto);
	return rsnano::dto_to_endpoint (endpoint_dto);
}

std::size_t nano::transport::tcp_listener::connections_count ()
{
	return rsnano::rsn_tcp_listener_connection_count (handle);
}

std::unique_ptr<nano::container_info_component> nano::transport::tcp_listener::collect_container_info (std::string const & name)
{
	// auto sizeof_element = sizeof (decltype (bootstrap_listener.connections)::value_type);
	size_t sizeof_element = 1;
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "connections", connection_count (), sizeof_element }));
	return composite;
}