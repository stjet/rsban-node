#pragma once

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
class BufferHandle;
class ErrorCodeDto;
class async_runtime;
}

namespace nano
{
class node;
class thread_pool;
class stats;
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

void async_read_adapter (void * context_a, rsnano::ErrorCodeDto const * error_a, std::size_t size_a);
void async_read_delete_context (void * context_a);

enum class socket_type
{
	undefined,
	bootstrap,
	realtime,
	realtime_response_server // special type for tcp channel response server
};

enum class socket_endpoint
{
	server, // Socket was created by accepting an incoming connection
	client // Socket was created by initiating an outgoing connection
};

/** Socket class for tcp clients and newly accepted connections */
class socket : public std::enable_shared_from_this<nano::transport::socket>
{
	friend class server_socket;

public:
	static std::size_t constexpr default_max_queue_size = 128;

	/**
	 * Constructor
	 * @param endpoint_type_a The endpoint's type: either server or client
	 */
	explicit socket (rsnano::async_runtime & async_rt_a, nano::transport::socket_endpoint endpoint_type_a, nano::stats & stats_a,
	std::shared_ptr<nano::thread_pool> const & workers_a,
	std::chrono::seconds default_timeout_a, std::chrono::seconds silent_connection_tolerance_time_a,
	std::chrono::seconds idle_timeout_a,
	std::shared_ptr<nano::node_observers>,
	std::size_t max_queue_size = default_max_queue_size);
	socket (rsnano::SocketHandle * handle_a);
	socket (nano::transport::socket const &) = delete;
	socket (nano::transport::socket &&) = delete;
	virtual ~socket ();

public:
	rsnano::SocketHandle * handle;
};

namespace socket_functions
{
	boost::asio::ip::network_v6 get_ipv6_subnet_address (boost::asio::ip::address_v6 const &, std::size_t);
}

std::shared_ptr<nano::transport::socket> create_client_socket (nano::node & node_a, std::size_t max_queue_size = socket::default_max_queue_size);
}
