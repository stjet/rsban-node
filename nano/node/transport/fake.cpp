#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/bandwidth_limiter.hpp"
#include "nano/node/transport/channel.hpp"

#include <nano/node/node.hpp>
#include <nano/node/transport/fake.hpp>

#include <boost/format.hpp>

#include <stdexcept>

namespace
{
rsnano::ChannelHandle * create_fake_channel (nano::node & node)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (node.network->endpoint ()) };
	return rsnano::rsn_channel_fake_create (
	std::chrono::steady_clock::now ().time_since_epoch ().count (),
	node.network->next_channel_id.fetch_add (1),
	&node.io_ctx,
	node.outbound_limiter.handle,
	node.stats->handle,
	&endpoint_dto,
	node.network_params.network.protocol_version);
}
}

nano::transport::fake::channel::channel (nano::node & node) :
	transport::channel{ create_fake_channel (node) }
{
	set_node_id (node.node_id.pub);
}

void nano::transport::fake::channel::send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_fake_send (handle, message_a.handle, nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

/**
 * The send function behaves like a null device, it throws the data away and returns success.
 */
void nano::transport::fake::channel::send_buffer (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_fake_send_buffer (handle, buffer_a.data (), buffer_a.size (), nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

std::size_t nano::transport::fake::channel::hash_code () const
{
	std::hash<::nano::endpoint> hash;
	return hash (get_endpoint ());
}

bool nano::transport::fake::channel::operator== (nano::transport::channel const & other_a) const
{
	return get_endpoint () == other_a.get_endpoint ();
}

bool nano::transport::fake::channel::operator== (nano::transport::fake::channel const & other_a) const
{
	return get_endpoint () == other_a.get_endpoint ();
}

std::string nano::transport::fake::channel::to_string () const
{
	return boost::str (boost::format ("%1%") % get_endpoint ());
}

void nano::transport::fake::channel::set_peering_endpoint (nano::endpoint endpoint)
{
	throw std::runtime_error ("setting peering endpoint not implemented for fake channel");
}

nano::endpoint nano::transport::fake::channel::get_peering_endpoint () const
{
	return get_endpoint ();
}

bool nano::transport::fake::channel::alive () const
{
	return rsnano::rsn_channel_is_alive (handle);
}

nano::endpoint nano::transport::fake::channel::get_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_fake_endpoint (handle, &dto);
	return rsnano::dto_to_udp_endpoint (dto);
}