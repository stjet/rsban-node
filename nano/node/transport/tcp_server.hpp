#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/tcp.hpp"
#include "nano/secure/common.hpp"

#include <nano/node/common.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/transport/socket.hpp>

#include <atomic>
#include <memory>

namespace rsnano
{
class BootstrapServerHandle;
class BootstrapServerWeakHandle;
}

namespace nano
{
class node_config;
class node_flags;
class network;
class ledger;
class block_processor;
class bootstrap_initiator;

namespace transport
{
	class tcp_channels;
	class tcp_server;
}

class tcp_server_weak_wrapper
{
public:
	tcp_server_weak_wrapper () = default;
	explicit tcp_server_weak_wrapper (std::shared_ptr<nano::transport::tcp_server> const & server);
	tcp_server_weak_wrapper (tcp_server_weak_wrapper const &);
	tcp_server_weak_wrapper (tcp_server_weak_wrapper &&) noexcept;
	~tcp_server_weak_wrapper ();
	tcp_server_weak_wrapper & operator= (tcp_server_weak_wrapper && other_a) noexcept;
	[[nodiscard]] std::shared_ptr<nano::transport::tcp_server> lock () const;

private:
	rsnano::BootstrapServerWeakHandle * handle{ nullptr };
};
class message;
class tcp_message_manager;
class syn_cookies;
}

namespace nano::transport
{
class message_deserializer;
class tcp_server;

/**
 * Server side portion of bootstrap sessions. Listens for new socket connections and spawns tcp_server objects when connected.
 */
class tcp_listener final : public std::enable_shared_from_this<nano::transport::tcp_listener>
{
public:
	tcp_listener (uint16_t, nano::node &, std::size_t);
	tcp_listener (tcp_listener const &) = delete;
	~tcp_listener ();
	void start (std::function<bool (std::shared_ptr<nano::transport::socket> const &, boost::system::error_code const &)> callback_a);
	void stop ();
	void accept_action (boost::system::error_code const &, std::shared_ptr<nano::transport::socket> const &);
	std::size_t connection_count ();
	std::size_t get_realtime_count ();
	nano::tcp_endpoint endpoint ();
	std::size_t connections_count ();
	rsnano::TcpListenerHandle * handle;
};

std::unique_ptr<container_info_component> collect_container_info (tcp_listener & bootstrap_listener, std::string const & name);

namespace bootstrap
{
	class message_deserializer;
};

class tcp_server final : public std::enable_shared_from_this<nano::transport::tcp_server>
{
public:
	tcp_server (
	rsnano::async_runtime & async_rt,
	std::shared_ptr<nano::transport::socket> const & socket_a,
	std::shared_ptr<nano::logger_mt> const & logger_a,
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
	bool is_stopped () const;
	std::size_t unique_id () const;
	void set_remote_node_id (nano::account account_a);
	nano::tcp_endpoint get_remote_endpoint () const;
	std::shared_ptr<nano::transport::socket> const get_socket () const;

	rsnano::TcpServerHandle * handle;
};
}
