#pragma once

#include <nano/boost/asio/ip/tcp.hpp>
#include <nano/boost/asio/strand.hpp>
#include <nano/lib/asio.hpp>

#include <boost/optional.hpp>

#include <chrono>
#include <deque>
#include <map>
#include <memory>
#include <vector>

namespace boost::asio::ip
{
class network_v6;
}

namespace rsnano
{
class SocketHandle;
}

namespace nano
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
class thread_pool;
class stat;
class logger_mt;
class node;

class tcp_socket_facade : public std::enable_shared_from_this<nano::tcp_socket_facade>
{
public:
	tcp_socket_facade (
	boost::asio::strand<boost::asio::io_context::executor_type> & strand,
	boost::asio::ip::tcp::socket & tcp_socket,
	boost::asio::io_context & io_ctx);

	void async_connect (boost::asio::ip::tcp::endpoint endpoint_a,
	std::function<void (boost::system::error_code const &)> callback_a);

	void async_read (std::shared_ptr<std::vector<uint8_t>> const & buffer_a, size_t len_a,
	std::function<void (boost::system::error_code const &, std::size_t)> callback_a);

	boost::asio::ip::tcp::endpoint remote_endpoint (boost::system::error_code & ec);

	void dispatch (std::function<void ()> callback_a);
	void close (boost::system::error_code & ec);

private:
	boost::asio::strand<boost::asio::io_context::executor_type> & strand;
	boost::asio::ip::tcp::socket & tcp_socket;
	boost::asio::io_context & io_ctx;
};

/** Socket class for tcp clients and newly accepted connections */
class socket : public std::enable_shared_from_this<nano::socket>
{
	friend class server_socket;

public:
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
	explicit socket (boost::asio::io_context & io_ctx_a, endpoint_type_t endpoint_type_a, nano::stat & stats_a, nano::logger_mt & logger_a, nano::thread_pool & workers_a, std::chrono::seconds default_timeout_a, std::chrono::seconds silent_connection_tolerance_time_a, bool network_timeout_logging_a);
	virtual ~socket ();
	void async_connect (boost::asio::ip::tcp::endpoint const &, std::function<void (boost::system::error_code const &)>);
	void async_read (std::shared_ptr<std::vector<uint8_t>> const &, std::size_t, std::function<void (boost::system::error_code const &, std::size_t)>);
	void async_write (nano::shared_const_buffer const &, std::function<void (boost::system::error_code const &, std::size_t)> = {});

	void close ();
	boost::asio::ip::tcp::endpoint remote_endpoint () const;
	boost::asio::ip::tcp::endpoint local_endpoint () const;
	/** Returns true if the socket has timed out */
	bool has_timed_out () const;
	/** This can be called to change the maximum idle time, e.g. based on the type of traffic detected. */
	void set_default_timeout_value (std::chrono::seconds);
	void set_timeout (std::chrono::seconds);
	void set_silent_connection_tolerance_time (std::chrono::seconds tolerance_time_a);
	bool max () const
	{
		return queue_size >= queue_size_max;
	}
	bool full () const
	{
		return queue_size >= queue_size_max * 2;
	}
	type_t type () const
	{
		return type_m;
	};
	void type_set (type_t type_a)
	{
		type_m = type_a;
	}
	endpoint_type_t endpoint_type () const
	{
		return endpoint_type_m;
	}
	bool is_realtime_connection ()
	{
		return type () == nano::socket::type_t::realtime || type () == nano::socket::type_t::realtime_response_server;
	}
	bool is_closed ();

protected:
	/** Holds the buffer and callback for queued writes */
	class queue_item
	{
	public:
		nano::shared_const_buffer buffer;
		std::function<void (boost::system::error_code const &, std::size_t)> callback;
	};

	boost::asio::strand<boost::asio::io_context::executor_type> strand;
	boost::asio::ip::tcp::socket tcp_socket;
	nano::logger_mt & logger;
	nano::stat & stats;
	boost::asio::io_context & io_ctx;
	nano::thread_pool & workers;

	/** The other end of the connection */
	boost::asio::ip::tcp::endpoint & get_remote ();

	/** The other end of the connection */
	boost::asio::ip::tcp::endpoint remote;

	/** Tracks number of blocks queued for delivery to the local socket send buffers.
	 *  Under normal circumstances, this should be zero.
	 *  Note that this is not the number of buffers queued to the peer, it is the number of buffers
	 *  queued up to enter the local TCP send buffer
	 *  socket buffer queue -> TCP send queue -> (network) -> TCP receive queue of peer
	 */
	std::atomic<std::size_t> queue_size{ 0 };

	void close_internal ();
	void checkup ();
	void set_default_timeout ();
	void set_last_completion ();
	void set_last_receive_time ();

private:
	type_t type_m{ type_t::undefined };
	endpoint_type_t endpoint_type_m;
	std::shared_ptr<tcp_socket_facade> tcp_socket_facade_m;

public:
	static std::size_t constexpr queue_size_max = 128;
	rsnano::SocketHandle * handle;
};

using address_socket_mmap = std::multimap<boost::asio::ip::address, std::weak_ptr<socket>>;

namespace socket_functions
{
	boost::asio::ip::network_v6 get_ipv6_subnet_address (boost::asio::ip::address_v6 const &, size_t);
	boost::asio::ip::address first_ipv6_subnet_address (boost::asio::ip::address_v6 const &, size_t);
	boost::asio::ip::address last_ipv6_subnet_address (boost::asio::ip::address_v6 const &, size_t);
	size_t count_subnetwork_connections (nano::address_socket_mmap const &, boost::asio::ip::address_v6 const &, size_t);
}

/** Socket class for TCP servers */
class server_socket final : public std::enable_shared_from_this<nano::server_socket>
{
public:
	/**
	 * Constructor
	 * @param node_a Owning node
	 * @param local_a Address and port to listen on
	 * @param max_connections_a Maximum number of concurrent connections
	 */
	explicit server_socket (nano::node & node_a, boost::asio::ip::tcp::endpoint local_a, std::size_t max_connections_a);
	/**Start accepting new connections */
	void start (boost::system::error_code &);
	/** Stop accepting new connections */
	void close ();
	/** Register callback for new connections. The callback must return true to keep accepting new connections. */
	void on_connection (std::function<bool (std::shared_ptr<nano::socket> const & new_connection, boost::system::error_code const &)>);
	uint16_t listening_port ()
	{
		return acceptor.local_endpoint ().port ();
	}
	nano::socket & get_socket ();

private:
	boost::asio::strand<boost::asio::io_context::executor_type> strand;
	nano::logger_mt & logger;
	nano::stat & stats;
	nano::socket socket;
	nano::thread_pool & workers;
	nano::node & node;
	nano::address_socket_mmap connections_per_address;
	boost::asio::ip::tcp::acceptor acceptor;
	boost::asio::ip::tcp::endpoint local;
	std::size_t max_inbound_connections;
	void evict_dead_connections ();
	void on_connection_requeue_delayed (std::function<bool (std::shared_ptr<nano::socket> const & new_connection, boost::system::error_code const &)>);
	/** Checks whether the maximum number of connections per IP was reached. If so, it returns true. */
	bool limit_reached_for_incoming_ip_connections (std::shared_ptr<nano::socket> const & new_connection);
	bool limit_reached_for_incoming_subnetwork_connections (std::shared_ptr<nano::socket> const & new_connection);
};

std::shared_ptr<nano::socket> create_client_socket (nano::node & node_a);
}
