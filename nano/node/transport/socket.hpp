#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/boost/asio/ip/tcp.hpp>
#include <nano/boost/asio/strand.hpp>
#include <nano/lib/asio.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/transport/traffic_type.hpp>

#include <chrono>
#include <map>
#include <memory>
#include <optional>
#include <queue>
#include <unordered_map>
#include <vector>

namespace boost::asio::ip
{
class network_v6;
}

namespace rsnano
{
class SocketHandle;
class SocketWeakHandle;
class BufferHandle;
class ErrorCodeDto;
class async_runtime;
}

namespace nano
{
class node;
class thread_pool;
class stats;
class logger_mt;
class node_observers;
}

namespace nano::transport
{
/** Policy to affect at which stage a buffer can be dropped */
enum class buffer_drop_policy
{
	/** Can be dropped by bandwidth limiter (default) */
	limiter,
	/** Should not be dropped by bandwidth limiter */
	no_limiter_drop,
	/** Should not be dropped by bandwidth limiter or socket write queue limiter */
	no_socket_drop
};

class server_socket;

class buffer_wrapper
{
public:
	buffer_wrapper (std::size_t len);
	buffer_wrapper (rsnano::BufferHandle * handle_a);
	buffer_wrapper (buffer_wrapper const &) = delete;
	buffer_wrapper (buffer_wrapper && other_a);
	~buffer_wrapper ();
	std::uint8_t * data ();
	std::size_t len () const;
	rsnano::BufferHandle * handle;
};

class tcp_socket_facade : public std::enable_shared_from_this<nano::transport::tcp_socket_facade>
{
public:
	tcp_socket_facade (boost::asio::io_context & io_ctx);
	~tcp_socket_facade ();

	void async_connect (boost::asio::ip::tcp::endpoint endpoint_a,
	std::function<void (boost::system::error_code const &)> callback_a);

	void async_read (std::shared_ptr<std::vector<uint8_t>> const & buffer_a, size_t len_a,
	std::function<void (boost::system::error_code const &, std::size_t)> callback_a);

	void async_read (std::shared_ptr<buffer_wrapper> const & buffer_a, size_t len_a,
	std::function<void (boost::system::error_code const &, std::size_t)> callback_a);

	void async_write (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> callback_a);
	bool running_in_this_thread ();
	void async_accept (
	boost::asio::ip::tcp::socket & client_socket,
	boost::asio::ip::tcp::endpoint & peer,
	std::function<void (boost::system::error_code const &)> callback_a);

	bool is_acceptor_open ();

	boost::asio::ip::tcp::endpoint remote_endpoint (boost::system::error_code & ec);

	void dispatch (std::function<void ()> callback_a);
	void post (std::function<void ()> callback_a);
	void close (boost::system::error_code & ec);
	void close_acceptor ();
	uint16_t listening_port ()
	{
		return acceptor.local_endpoint ().port ();
	}

	void open (boost::asio::ip::tcp::endpoint & local, boost::system::error_code & ec_a);

	boost::asio::strand<boost::asio::io_context::executor_type> strand;
	boost::asio::ip::tcp::socket tcp_socket;
	boost::asio::io_context & io_ctx;
	boost::asio::ip::tcp::acceptor acceptor;

private:
	std::atomic<bool> closed{ false };
};

class tcp_socket_facade_factory
{
public:
	tcp_socket_facade_factory (boost::asio::io_context & io_ctx);
	std::shared_ptr<nano::transport::tcp_socket_facade> create_socket ();

private:
	boost::asio::io_context & io_ctx;
};

void async_read_adapter (void * context_a, rsnano::ErrorCodeDto const * error_a, std::size_t size_a);
void async_read_delete_context (void * context_a);

/** Socket class for tcp clients and newly accepted connections */
class socket : public std::enable_shared_from_this<nano::transport::socket>
{
	friend class server_socket;

public:
	static std::size_t constexpr default_max_queue_size = 128;

	enum class type_t
	{
		undefined,
		bootstrap,
		realtime,
		realtime_response_server // special type for tcp channel response server
	};

	enum class endpoint_type_t
	{
		server,
		client
	};

	/**
	 * Constructor
	 * @param endpoint_type_a The endpoint's type: either server or client
	 */
	explicit socket (rsnano::async_runtime & async_rt_a, endpoint_type_t endpoint_type_a, nano::stats & stats_a,
	std::shared_ptr<nano::logger_mt> & logger_a, std::shared_ptr<nano::thread_pool> const & workers_a,
	std::chrono::seconds default_timeout_a, std::chrono::seconds silent_connection_tolerance_time_a,
	std::chrono::seconds idle_timeout_a, bool network_timeout_logging_a,
	std::shared_ptr<nano::node_observers>,
	std::size_t max_queue_size = default_max_queue_size);
	socket (rsnano::SocketHandle * handle_a);
	socket (nano::transport::socket const &) = delete;
	socket (nano::transport::socket &&) = delete;
	virtual ~socket ();

	void start ();

	void async_connect (boost::asio::ip::tcp::endpoint const &, std::function<void (boost::system::error_code const &)>);
	void async_read (std::shared_ptr<std::vector<uint8_t>> const &, std::size_t, std::function<void (boost::system::error_code const &, std::size_t)>);
	void async_read (std::shared_ptr<buffer_wrapper> const &, std::size_t, std::function<void (boost::system::error_code const &, std::size_t)>);
	void async_write (nano::shared_const_buffer const &, std::function<void (boost::system::error_code const &, std::size_t)> = {}, nano::transport::traffic_type = nano::transport::traffic_type::generic);

	virtual void close ();
	boost::asio::ip::tcp::endpoint remote_endpoint () const;
	boost::asio::ip::tcp::endpoint local_endpoint () const;
	/** Returns true if the socket has timed out */
	bool has_timed_out () const;
	/** This can be called to change the maximum idle time, e.g. based on the type of traffic detected. */
	void set_default_timeout_value (std::chrono::seconds);
	std::chrono::seconds get_default_timeout_value () const;
	void set_timeout (std::chrono::seconds);
	void set_silent_connection_tolerance_time (std::chrono::seconds tolerance_time_a);
	bool max (nano::transport::traffic_type = nano::transport::traffic_type::generic) const;
	bool full (nano::transport::traffic_type = nano::transport::traffic_type::generic) const;
	type_t type () const;
	void type_set (type_t type_a);
	endpoint_type_t endpoint_type () const;
	bool is_realtime_connection ()
	{
		return type () == nano::transport::socket::type_t::realtime || type () == nano::transport::socket::type_t::realtime_response_server;
	}
	bool is_bootstrap_connection ();
	bool is_closed ();
	bool alive () const;

private:
	/** The other end of the connection */
	boost::asio::ip::tcp::endpoint & get_remote ();

	/** The other end of the connection */
	boost::asio::ip::tcp::endpoint remote;

	void close_internal ();
	void checkup ();

public:
	rsnano::SocketHandle * handle;
};

class weak_socket_wrapper
{
public:
	weak_socket_wrapper (rsnano::SocketWeakHandle * handle);
	weak_socket_wrapper (weak_socket_wrapper const &) = delete;
	weak_socket_wrapper (weak_socket_wrapper &&) = delete;
	weak_socket_wrapper (std::shared_ptr<nano::transport::socket> & socket);
	~weak_socket_wrapper ();
	std::shared_ptr<nano::transport::socket> lock ();
	bool expired () const;

private:
	rsnano::SocketWeakHandle * handle;
};

std::string socket_type_to_string (socket::type_t type);

using address_socket_mmap = std::multimap<boost::asio::ip::address, weak_socket_wrapper>;

namespace socket_functions
{
	boost::asio::ip::network_v6 get_ipv6_subnet_address (boost::asio::ip::address_v6 const &, std::size_t);
}

/** Socket class for TCP servers */
class server_socket final : public std::enable_shared_from_this<nano::transport::server_socket>
{
public:
	/**
	 * Constructor
	 * @param node_a Owning node
	 * @param local_a Address and port to listen on
	 * @param max_connections_a Maximum number of concurrent connections
	 */
	explicit server_socket (nano::node & node_a, boost::asio::ip::tcp::endpoint local_a, std::size_t max_connections_a);
	server_socket (server_socket const &) = delete;
	~server_socket ();
	/**Start accepting new connections */
	void start (boost::system::error_code &);
	/** Stop accepting new connections */
	void close ();
	/** Register callback for new connections. The callback must return true to keep accepting new connections. */
	void on_connection (std::function<bool (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const &)>);
	uint16_t listening_port ()
	{
		return socket_facade->listening_port ();
	}

private:
	std::shared_ptr<nano::transport::tcp_socket_facade> socket_facade;
	nano::transport::socket socket;
	boost::asio::ip::tcp::endpoint local;
	rsnano::ServerSocketHandle * handle;
};

std::shared_ptr<nano::transport::socket> create_client_socket (nano::node & node_a, std::size_t max_queue_size = socket::default_max_queue_size);
}
