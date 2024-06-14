#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_connections.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>

#include <memory>

constexpr double nano::bootstrap_limits::bootstrap_connection_scale_target_blocks;
constexpr double nano::bootstrap_limits::bootstrap_minimum_blocks_per_sec;
constexpr double nano::bootstrap_limits::bootstrap_minimum_termination_time_sec;
constexpr unsigned nano::bootstrap_limits::bootstrap_max_new_connections;
constexpr unsigned nano::bootstrap_limits::requeued_pulls_processed_blocks_factor;

nano::bootstrap_client::bootstrap_client (rsnano::BootstrapClientHandle * handle_a) :
	handle{ handle_a }
{
}

nano::bootstrap_client::~bootstrap_client ()
{
	rsnano::rsn_bootstrap_client_destroy (handle);
}

std::shared_ptr<nano::transport::socket> nano::bootstrap_client::get_socket () const
{
	return std::make_shared<nano::transport::socket> (rsnano::rsn_bootstrap_client_socket (handle));
}

nano::bootstrap_connections::bootstrap_connections (rsnano::BootstrapConnectionsHandle * handle) :
	handle{ handle }
{
}

nano::bootstrap_connections::bootstrap_connections (nano::node & node_a, nano::bootstrap_initiator & initiator)
{
	auto config_dto{ node_a.config->to_dto () };
	auto params_dto{ node_a.network_params.to_dto () };
	handle = rsnano::rsn_bootstrap_connections_create (initiator.attempts.handle, &config_dto,
	node_a.flags.handle, node_a.network->tcp_channels->handle,
	node_a.async_rt.handle, node_a.workers->handle, &params_dto,
	new std::weak_ptr<nano::node_observers> (node_a.observers),
	node_a.stats->handle, node_a.outbound_limiter.handle, node_a.block_processor.handle,
	initiator.cache.handle);
}

nano::bootstrap_connections::~bootstrap_connections ()
{
	rsnano::rsn_bootstrap_connections_drop (handle);
}

unsigned nano::bootstrap_connections::target_connections (std::size_t pulls_remaining, std::size_t attempts_count) const
{
	return rsnano::rsn_bootstrap_connections_target_connections (handle, pulls_remaining, attempts_count);
}

void nano::bootstrap_connections::bootstrap_status (boost::property_tree::ptree & connections, std::size_t attempts_count)
{
	rsnano::rsn_bootstrap_connections_bootstrap_status (handle, &connections, attempts_count);
}

unsigned nano::bootstrap_connections::get_connections_count () const
{
	return rsnano::rsn_bootstrap_connections_connections_count (handle);
}
