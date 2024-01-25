#include "nano/lib/logger_mt.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/node/bootstrap/bootstrap.hpp"
#include "nano/secure/common.hpp"
#include "nano/secure/ledger.hpp"

#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/node/transport/tcp_server.hpp>

#include <boost/format.hpp>

nano::tcp_server_weak_wrapper::tcp_server_weak_wrapper (std::shared_ptr<nano::transport::tcp_server> const & server) :
	handle{ rsnano::rsn_bootstrap_server_get_weak (server->handle) }
{
}

nano::tcp_server_weak_wrapper::tcp_server_weak_wrapper (tcp_server_weak_wrapper const & other_a) :
	handle{ rsnano::rsn_bootstrap_server_copy_weak (other_a.handle) }
{
}

nano::tcp_server_weak_wrapper::tcp_server_weak_wrapper (tcp_server_weak_wrapper && other_a) noexcept :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::tcp_server_weak_wrapper::~tcp_server_weak_wrapper ()
{
	if (handle)
		rsnano::rsn_bootstrap_server_destroy_weak (handle);
}

nano::tcp_server_weak_wrapper & nano::tcp_server_weak_wrapper::operator= (tcp_server_weak_wrapper && other_a) noexcept
{
	if (handle)
		rsnano::rsn_bootstrap_server_destroy_weak (handle);
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}

std::shared_ptr<nano::transport::tcp_server> nano::tcp_server_weak_wrapper::lock () const
{
	auto server_handle = rsnano::rsn_bootstrap_server_lock_weak (handle);
	if (server_handle)
		return std::make_shared<nano::transport::tcp_server> (server_handle);

	return {};
}

/*
 * tcp_listener
 */

nano::transport::tcp_listener::tcp_listener (uint16_t port_a, nano::node & node_a, std::size_t max_inbound_connections) :
	config{ node_a.config },
	logger{ node_a.logger },
	tcp_channels{ node_a.network->tcp_channels },
	syn_cookies{ node_a.network->syn_cookies },
	node (node_a),
	port (port_a),
	max_inbound_connections{ max_inbound_connections }
{
	auto config_dto{ node_a.config->to_dto () };
	auto logger_handle{ nano::to_logger_handle (node_a.logger) };
	handle = rsnano::rsn_tcp_listener_create (
	port_a,
	max_inbound_connections,
	&config_dto,
	logger_handle,
	node_a.network->tcp_channels->handle,
	node_a.network->syn_cookies->handle);
}

nano::transport::tcp_listener::~tcp_listener ()
{
	rsnano::rsn_tcp_listener_destroy (handle);
}

void nano::transport::tcp_listener::start (std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> callback_a)
{
	nano::lock_guard<nano::mutex> lock{ mutex };
	on = true;
	listening_socket = std::make_shared<nano::transport::server_socket> (node, boost::asio::ip::tcp::endpoint (boost::asio::ip::address_v6::any (), port), max_inbound_connections);
	boost::system::error_code ec;
	listening_socket->start (ec);
	if (ec)
	{
		logger->always_log (boost::str (boost::format ("Network: Error while binding for incoming TCP/bootstrap on port %1%: %2%") % listening_socket->listening_port () % ec.message ()));
		throw std::runtime_error (ec.message ());
	}

	// the user can either specify a port value in the config or it can leave the choice up to the OS:
	// (1): port specified
	// (2): port not specified
	//
	const auto listening_port = listening_socket->listening_port ();
	{
		// (1) -- nothing to do, just check that port values match everywhere
		//
		if (port == listening_port)
		{
		}
		// (2) -- OS port choice happened at TCP socket bind time, so propagate this port value back;
		// the propagation is done here for the `tcp_listener` itself, whereas for `network`, the node does it
		// after calling `tcp_listener.start ()`
		//
		else
		{
			port = listening_port;
		}
	}

	listening_socket->on_connection (callback_a);
}

void nano::transport::tcp_listener::stop ()
{
	decltype (connections) connections_l;
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		on = false;
		connections_l.swap (connections);
	}
	if (listening_socket)
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		listening_socket->close ();
		listening_socket = nullptr;
	}
}

std::size_t nano::transport::tcp_listener::connection_count ()
{
	nano::lock_guard<nano::mutex> lock{ mutex };
	return connections.size ();
}

void nano::transport::tcp_listener::erase_connection (std::uintptr_t conn_ptr)
{
	nano::lock_guard<nano::mutex> lock (mutex);
	connections.erase (conn_ptr);
}

std::size_t nano::transport::tcp_listener::get_bootstrap_count ()
{
	return bootstrap_count;
}

void nano::transport::tcp_listener::inc_bootstrap_count ()
{
	++bootstrap_count;
}

void nano::transport::tcp_listener::dec_bootstrap_count ()
{
	--bootstrap_count;
}

std::size_t nano::transport::tcp_listener::get_realtime_count ()
{
	return realtime_count;
}

void nano::transport::tcp_listener::inc_realtime_count ()
{
	++realtime_count;
}

void nano::transport::tcp_listener::dec_realtime_count ()
{
	--realtime_count;
}

void nano::transport::tcp_listener::tcp_server_timeout (std::uintptr_t inner_ptr)
{
	if (config->logging.bulk_pull_logging ())
	{
		logger->try_log ("Closing incoming tcp / bootstrap server by timeout");
	}
	{
		erase_connection (inner_ptr);
	}
}

void nano::transport::tcp_listener::tcp_server_exited (nano::transport::socket::type_t type_a, std::uintptr_t inner_ptr_a, nano::tcp_endpoint const & endpoint_a)
{
	if (config->logging.bulk_pull_logging ())
	{
		logger->try_log ("Exiting incoming TCP/bootstrap server");
	}
	if (type_a == nano::transport::socket::type_t::bootstrap)
	{
		dec_bootstrap_count ();
	}
	else if (type_a == nano::transport::socket::type_t::realtime)
	{
		dec_realtime_count ();
		// Clear temporary channel
		tcp_channels->erase_temporary_channel (endpoint_a);
	}
	erase_connection (inner_ptr_a);
}

void nano::transport::tcp_listener::accept_action (boost::system::error_code const & ec, std::shared_ptr<nano::transport::socket> const & socket_a)
{
	if (!tcp_channels->excluded_peers ().check (socket_a->remote_endpoint ()))
	{
		auto req_resp_visitor_factory = std::make_shared<nano::transport::request_response_visitor_factory> (node);
		auto server (std::make_shared<nano::transport::tcp_server> (
		node.async_rt, socket_a, logger,
		*node.stats, node.flags, *config,
		node.tcp_listener, req_resp_visitor_factory,
		node.bootstrap_workers,
		*tcp_channels->publish_filter,
		tcp_channels->tcp_message_manager,
		*syn_cookies,
		node.ledger,
		node.block_processor,
		node.bootstrap_initiator,
		node.node_id,
		true));
		nano::lock_guard<nano::mutex> lock{ mutex };
		connections[server->unique_id ()] = nano::tcp_server_weak_wrapper (server);
		server->start ();
	}
	else
	{
		node.stats->inc (nano::stat::type::tcp, nano::stat::detail::tcp_excluded);
		if (config->logging.network_rejected_logging ())
		{
			logger->try_log ("Rejected connection from excluded peer ", socket_a->remote_endpoint ());
		}
	}
}

boost::asio::ip::tcp::endpoint nano::transport::tcp_listener::endpoint ()
{
	nano::lock_guard<nano::mutex> lock{ mutex };
	if (on && listening_socket)
	{
		return { boost::asio::ip::address_v6::loopback (), port };
	}
	else
	{
		return { boost::asio::ip::address_v6::loopback (), 0 };
	}
}

std::size_t nano::transport::tcp_listener::connections_count ()
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return connections.size ();
}

std::unique_ptr<nano::container_info_component> nano::transport::collect_container_info (nano::transport::tcp_listener & bootstrap_listener, std::string const & name)
{
	//auto sizeof_element = sizeof (decltype (bootstrap_listener.connections)::value_type);
	size_t sizeof_element = 1;
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "connections", bootstrap_listener.connection_count (), sizeof_element }));
	return composite;
}

nano::transport::tcp_server::tcp_server (
rsnano::async_runtime & async_rt,
std::shared_ptr<nano::transport::socket> const & socket_a,
std::shared_ptr<nano::logger_mt> const & logger_a,
nano::stats const & stats_a,
nano::node_flags const & flags_a,
nano::node_config const & config_a,
std::shared_ptr<nano::tcp_server_observer> const & observer_a,
std::shared_ptr<nano::transport::request_response_visitor_factory> visitor_factory_a,
std::shared_ptr<nano::thread_pool> const & bootstrap_workers_a,
nano::network_filter const & publish_filter_a,
nano::tcp_message_manager & tcp_message_manager_a,
nano::syn_cookies & syn_cookies_a,
nano::ledger & ledger_a,
nano::block_processor & block_processor_a,
nano::bootstrap_initiator & bootstrap_initiator_a,
nano::keypair & node_id_a,
bool allow_bootstrap_a)
{
	auto config_dto{ config_a.to_dto () };
	auto observer_handle = new std::weak_ptr<nano::tcp_server_observer> (observer_a);
	auto network_dto{ config_a.network_params.to_dto () };
	rsnano::CreateTcpServerParams params;
	params.async_rt = async_rt.handle;
	params.socket = socket_a->handle;
	params.config = &config_dto;
	params.logger = nano::to_logger_handle (logger_a);
	params.observer = observer_handle;
	params.publish_filter = publish_filter_a.handle;
	params.network = &network_dto;
	params.disable_bootstrap_listener = flags_a.disable_bootstrap_listener ();
	params.connections_max = config_a.bootstrap_connections_max;
	params.stats = stats_a.handle;
	params.disable_bootstrap_bulk_pull_server = flags_a.disable_bootstrap_bulk_pull_server ();
	params.disable_tcp_realtime = flags_a.disable_tcp_realtime ();
	params.request_response_visitor_factory = visitor_factory_a->handle;
	params.tcp_message_manager = tcp_message_manager_a.handle;
	params.allow_bootstrap = allow_bootstrap_a;
	handle = rsnano::rsn_bootstrap_server_create (&params);
	debug_assert (socket_a != nullptr);
}

nano::transport::tcp_server::tcp_server (rsnano::TcpServerHandle * handle_a) :
	handle{ handle_a }
{
}

nano::transport::tcp_server::~tcp_server ()
{
	rsnano::rsn_bootstrap_server_destroy (handle);
}

void nano::transport::tcp_server::start ()
{
	rsnano::rsn_bootstrap_server_start (handle);
}

void nano::transport::tcp_server::stop ()
{
	rsnano::rsn_bootstrap_server_stop (handle);
}

// TODO: We could periodically call this (from a dedicated timeout thread for eg.) but socket already handles timeouts,
//  and since we only ever store tcp_server as weak_ptr, socket timeout will automatically trigger tcp_server cleanup
void nano::transport::tcp_server::timeout ()
{
	rsnano::rsn_bootstrap_server_timeout (handle);
}

/*
 * Bootstrap
 */

namespace
{
rsnano::RequestResponseVisitorFactoryHandle * create_request_response_message_visitor_factory (nano::node & node_a)
{
	auto config_dto{ node_a.config->to_dto () };
	auto network_dto{ node_a.config->network_params.to_dto () };
	rsnano::RequestResponseVisitorFactoryParams params;
	params.async_rt = node_a.async_rt.handle;
	params.config = &config_dto;
	params.logger = nano::to_logger_handle (node_a.logger);
	params.workers = node_a.bootstrap_workers->handle;
	params.network = &network_dto;
	params.stats = node_a.stats->handle;
	params.syn_cookies = node_a.network->syn_cookies->handle;
	params.node_id_prv = node_a.node_id.prv.bytes.data ();
	params.ledger = node_a.ledger.handle;
	params.block_processor = node_a.block_processor.handle;
	params.bootstrap_initiator = node_a.bootstrap_initiator.handle;
	params.flags = node_a.flags.handle;

	return rsnano::rsn_request_response_visitor_factory_create (&params);
}
}

nano::transport::request_response_visitor_factory::request_response_visitor_factory (nano::node & node_a) :
	handle{ create_request_response_message_visitor_factory (node_a) }
{
}

nano::transport::request_response_visitor_factory::~request_response_visitor_factory ()
{
	rsnano::rsn_request_response_visitor_factory_destroy (handle);
}

bool nano::transport::tcp_server::is_stopped () const
{
	return rsnano::rsn_bootstrap_server_is_stopped (handle);
}

std::uintptr_t nano::transport::tcp_server::unique_id () const
{
	return rsnano::rsn_bootstrap_server_unique_id (handle);
}

void nano::transport::tcp_server::set_remote_node_id (nano::account account_a)
{
	rsnano::rsn_bootstrap_server_set_remote_node_id (handle, account_a.bytes.data ());
}

nano::tcp_endpoint nano::transport::tcp_server::get_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_bootstrap_server_remote_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

std::shared_ptr<nano::transport::socket> const nano::transport::tcp_server::get_socket () const
{
	auto socket_handle = rsnano::rsn_bootstrap_server_socket (handle);
	return std::make_shared<nano::transport::socket> (socket_handle);
}
