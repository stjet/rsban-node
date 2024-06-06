#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/node/transport/tcp_server.hpp>

nano::transport::tcp_config::tcp_config (rsnano::TcpConfigDto const & dto) :
	max_inbound_connections{ dto.max_inbound_connections },
	max_outbound_connections{ dto.max_outbound_connections },
	max_attempts{ dto.max_attempts },
	max_attempts_per_ip{ dto.max_attempts_per_ip },
	connect_timeout{ dto.connect_timeout_s }
{
}

rsnano::TcpConfigDto nano::transport::tcp_config::to_dto () const
{
	rsnano::TcpConfigDto dto;
	dto.max_inbound_connections = max_inbound_connections;
	dto.max_outbound_connections = max_outbound_connections;
	dto.max_attempts = max_attempts;
	dto.max_attempts_per_ip = max_attempts_per_ip;
	dto.connect_timeout_s = connect_timeout.count ();
	return dto;
}

/*
 * tcp_listener
 */

nano::transport::tcp_listener::tcp_listener (rsnano::TcpListenerHandle * handle) :
	handle{ handle }
{
}

nano::transport::tcp_listener::~tcp_listener ()
{
	rsnano::rsn_tcp_listener_destroy (handle);
}

std::size_t nano::transport::tcp_listener::get_realtime_count ()
{
	return rsnano::rsn_tcp_listener_realtime_count (handle);
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
