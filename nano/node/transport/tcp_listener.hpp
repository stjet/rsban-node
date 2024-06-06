#pragma once

#include "nano/lib/rsnano.hpp"
#include <nano/node/common.hpp>

namespace rsnano
{
class TcpListenerHandle;
}

namespace nano::transport
{
class socket;
class tcp_server;

class tcp_config
{
public:
	tcp_config() = default;
	explicit tcp_config(rsnano::TcpConfigDto const & dto);

	explicit tcp_config (nano::network_constants const & network)
	{
		if (network.is_dev_network ())
		{
			max_inbound_connections = 128;
			max_outbound_connections = 128;
			max_attempts = 128;
			max_attempts_per_ip = 128;
			connect_timeout = std::chrono::seconds{ 5 };
		}
	}

	rsnano::TcpConfigDto to_dto() const;

	size_t max_inbound_connections{ 2048 };
	size_t max_outbound_connections{ 2048 };
	size_t max_attempts{ 60 };
	size_t max_attempts_per_ip { 1};
	std::chrono::seconds connect_timeout{ 60 };
};

/**
 * Server side portion of bootstrap sessions. Listens for new socket connections and spawns tcp_server objects when connected.
 */
class tcp_listener final : public std::enable_shared_from_this<tcp_listener>
{
public:
	tcp_listener (uint16_t, tcp_config const & config, nano::node &);
	tcp_listener (rsnano::TcpListenerHandle * handle);
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
}
