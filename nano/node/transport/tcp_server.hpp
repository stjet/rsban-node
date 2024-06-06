#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/tcp.hpp"
#include "nano/secure/common.hpp"

#include <nano/node/common.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/transport/socket.hpp>

#include <memory>

namespace nano
{
class node_config;
class node_flags;
class network;
class ledger;
class block_processor;
class bootstrap_initiator;
class tcp_message_manager;
class syn_cookies;
}

namespace nano::transport
{
class tcp_server final : public std::enable_shared_from_this<nano::transport::tcp_server>
{
public:
	tcp_server (
	rsnano::async_runtime & async_rt,
	nano::transport::tcp_channels & channels,
	std::shared_ptr<nano::transport::socket> const & socket_a,
	nano::stats const & stats_a,
	nano::node_flags const & flags_a,
	nano::node_config const & config_a,
	std::shared_ptr<nano::transport::tcp_listener> const & observer_a,
	std::shared_ptr<nano::transport::request_response_visitor_factory> visitor_factory_a,
	std::shared_ptr<nano::thread_pool> const & workers_a,
	nano::network_filter const & publish_filter_a,
	nano::tcp_message_manager & tcp_message_manager_a,
	nano::syn_cookies & syn_cookies_a,
	nano::ledger & ledger_a,
	nano::block_processor & block_processor_a,
	nano::bootstrap_initiator & bootstrap_initiator_a,
	nano::keypair & node_id_a,
	bool allow_bootstrap_a = true);

	explicit tcp_server (rsnano::TcpServerHandle * handle_a);
	tcp_server (nano::transport::tcp_server const &) = delete;
	tcp_server (nano::transport::tcp_server &&) = delete;
	~tcp_server ();

	void start ();
	void stop ();

	void timeout ();
	std::optional<nano::keepalive> get_last_keepalive () const;
	bool is_stopped () const;
	std::size_t unique_id () const;
	nano::tcp_endpoint get_remote_endpoint () const;
	std::shared_ptr<nano::transport::socket> const get_socket () const;

	rsnano::TcpServerHandle * handle;
};
}
