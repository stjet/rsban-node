#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/node/transport/tcp_server.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

#include <optional>

nano::transport::tcp_server::tcp_server (
rsnano::async_runtime & async_rt,
std::shared_ptr<nano::transport::socket> const & socket_a,
nano::stats const & stats_a,
nano::node_flags const & flags_a,
nano::node_config const & config_a,
std::shared_ptr<nano::transport::tcp_listener> const & observer_a,
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
	auto network_dto{ config_a.network_params.to_dto () };
	rsnano::CreateTcpServerParams params;
	params.async_rt = async_rt.handle;
	params.socket = socket_a->handle;
	params.config = &config_dto;
	params.observer = observer_a->handle;
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
	params.syn_cookies = syn_cookies_a.handle;
	params.node_id_priv = node_id_a.prv.bytes.data ();
	handle = rsnano::rsn_tcp_server_create (&params);
	debug_assert (socket_a != nullptr);
}

nano::transport::tcp_server::tcp_server (rsnano::TcpServerHandle * handle_a) :
	handle{ handle_a }
{
}

nano::transport::tcp_server::~tcp_server ()
{
	rsnano::rsn_tcp_server_destroy (handle);
}

void nano::transport::tcp_server::start ()
{
	rsnano::rsn_tcp_server_start (handle);
}

void nano::transport::tcp_server::stop ()
{
	rsnano::rsn_tcp_server_stop (handle);
}

// TODO: We could periodically call this (from a dedicated timeout thread for eg.) but socket already handles timeouts,
//  and since we only ever store tcp_server as weak_ptr, socket timeout will automatically trigger tcp_server cleanup
void nano::transport::tcp_server::timeout ()
{
	rsnano::rsn_tcp_server_timeout (handle);
}

std::optional<nano::keepalive> nano::transport::tcp_server::get_last_keepalive () const
{
	auto message = rsnano::rsn_tcp_server_get_last_keepalive (handle);
	auto result = nano::message_handle_to_message (message);
	if (result == nullptr)
	{
		return {};
	}
	else
	{
		nano::keepalive keepalive{ *static_cast<nano::keepalive *> (result.get ()) };
		return { keepalive };
	}
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
	return rsnano::rsn_tcp_server_is_stopped (handle);
}

std::uintptr_t nano::transport::tcp_server::unique_id () const
{
	return rsnano::rsn_tcp_server_unique_id (handle);
}

void nano::transport::tcp_server::set_remote_node_id (nano::account account_a)
{
	rsnano::rsn_tcp_server_set_remote_node_id (handle, account_a.bytes.data ());
}

nano::tcp_endpoint nano::transport::tcp_server::get_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_tcp_server_remote_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

std::shared_ptr<nano::transport::socket> const nano::transport::tcp_server::get_socket () const
{
	auto socket_handle = rsnano::rsn_tcp_server_socket (handle);
	return std::make_shared<nano::transport::socket> (socket_handle);
}
