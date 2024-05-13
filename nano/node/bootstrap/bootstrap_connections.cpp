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

double nano::bootstrap_client::sample_block_rate ()
{
	return rsnano::rsn_bootstrap_client_sample_block_rate (handle);
}

void nano::bootstrap_client::set_start_time ()
{
	rsnano::rsn_bootstrap_client_set_start_time (handle);
}

double nano::bootstrap_client::elapsed_seconds () const
{
	return rsnano::rsn_bootstrap_client_elapsed_seconds (handle);
}

void nano::bootstrap_client::stop (bool force)
{
	rsnano::rsn_bootstrap_client_stop (handle, force);
}

void nano::bootstrap_client::async_read (std::size_t size_a, std::function<void (boost::system::error_code const &, std::size_t)> callback_a)
{
	auto cb_wrapper = new std::function<void (boost::system::error_code const &, std::size_t)> ([callback = std::move (callback_a), this_l = shared_from_this ()] (boost::system::error_code const & ec, std::size_t size) {
		callback (ec, size);
	});
	rsnano::rsn_bootstrap_client_read (handle, size_a, nano::transport::async_read_adapter, nano::transport::async_read_delete_context, cb_wrapper);
}

uint8_t * nano::bootstrap_client::get_receive_buffer ()
{
	buffer.resize (rsnano::rsn_bootstrap_client_receive_buffer_size (handle));
	rsnano::rsn_bootstrap_client_receive_buffer (handle, buffer.data (), buffer.size ());
	return buffer.data ();
}

nano::tcp_endpoint nano::bootstrap_client::remote_endpoint () const
{
	rsnano::EndpointDto result;
	rsnano::rsn_bootstrap_client_remote_endpoint (handle, &result);
	return rsnano::dto_to_endpoint (result);
}

std::string nano::bootstrap_client::channel_string () const
{
	rsnano::StringDto dto;
	rsnano::rsn_bootstrap_client_channel_string (handle, &dto);
	return rsnano::convert_dto_to_string (dto);
}

void nano::bootstrap_client::send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_bootstrap_client_send (handle, message_a.handle, nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

void nano::bootstrap_client::send_buffer (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_bootstrap_client_send_buffer (handle, buffer_a.data (), buffer_a.size (), nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (policy_a), static_cast<uint8_t> (traffic_type));
}

nano::tcp_endpoint nano::bootstrap_client::get_tcp_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_bootstrap_client_tcp_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

void nano::bootstrap_client::close_socket ()
{
	rsnano::rsn_bootstrap_client_close_socket (handle);
}

void nano::bootstrap_client::set_timeout (std::chrono::seconds timeout_a)
{
	rsnano::rsn_bootstrap_client_set_timeout (handle, timeout_a.count ());
}

std::shared_ptr<nano::transport::socket> nano::bootstrap_client::get_socket () const
{
	return std::make_shared<nano::transport::socket> (rsnano::rsn_bootstrap_client_socket (handle));
}

uint64_t nano::bootstrap_client::inc_block_count ()
{
	return rsnano::rsn_bootstrap_client_inc_block_count (handle);
}

uint64_t nano::bootstrap_client::get_block_count () const
{
	return rsnano::rsn_bootstrap_client_block_count (handle);
}
double nano::bootstrap_client::get_block_rate () const
{
	return rsnano::rsn_bootstrap_client_block_rate (handle);
}
bool nano::bootstrap_client::get_pending_stop () const
{
	return rsnano::rsn_bootstrap_client_pending_stop (handle);
}
bool nano::bootstrap_client::get_hard_stop () const
{
	return rsnano::rsn_bootstrap_client_hard_stop (handle);
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
	initiator.handle, initiator.cache.handle);
}

nano::bootstrap_connections::~bootstrap_connections ()
{
	rsnano::rsn_bootstrap_connections_drop (handle);
}

void nano::bootstrap_connections::add_connection (nano::endpoint const & endpoint_a)
{
	auto dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	rsnano::rsn_bootstrap_connections_add_connection (handle, &dto);
}

unsigned nano::bootstrap_connections::target_connections (std::size_t pulls_remaining, std::size_t attempts_count) const
{
	return rsnano::rsn_bootstrap_connections_target_connections (handle, pulls_remaining, attempts_count);
}

void nano::bootstrap_connections::clear_pulls (uint64_t bootstrap_id_a)
{
	rsnano::rsn_bootstrap_connections_clear_pulls (handle, bootstrap_id_a);
}

void nano::bootstrap_connections::run ()
{
	rsnano::rsn_bootstrap_connections_run (handle);
}

void nano::bootstrap_connections::stop ()
{
	rsnano::rsn_bootstrap_connections_stop (handle);
}

void nano::bootstrap_connections::bootstrap_status (boost::property_tree::ptree & connections, std::size_t attempts_count)
{
	rsnano::rsn_bootstrap_connections_bootstrap_status (handle, &connections, attempts_count);
}

unsigned nano::bootstrap_connections::get_connections_count () const
{
	return rsnano::rsn_bootstrap_connections_connections_count (handle);
}
