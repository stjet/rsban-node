#include <nano/lib/rsnano.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <boost/asio/ip/address.hpp>
#include <boost/asio/ip/address_v4.hpp>
#include <boost/asio/ip/address_v6.hpp>
#include <boost/format.hpp>

nano::transport::channel::channel (rsnano::ChannelHandle * handle_a) :
	handle (handle_a)
{
}

nano::transport::channel::~channel ()
{
	rsnano::rsn_channel_destroy (handle);
}

bool nano::transport::channel::is_temporary () const
{
	return rsnano::rsn_channel_is_temporary (handle);
}

void nano::transport::channel::set_temporary (bool temporary)
{
	rsnano::rsn_channel_set_temporary (handle, temporary);
}

std::chrono::steady_clock::time_point nano::transport::channel::get_last_bootstrap_attempt () const
{
	auto value = rsnano::rsn_channel_get_last_bootstrap_attempt (handle);
	return std::chrono::steady_clock::time_point (std::chrono::steady_clock::duration (value));
}

void nano::transport::channel::set_last_bootstrap_attempt (std::chrono::steady_clock::time_point const time_a)
{
	rsnano::rsn_channel_set_last_bootstrap_attempt (handle, time_a.time_since_epoch ().count ());
}

std::chrono::steady_clock::time_point nano::transport::channel::get_last_packet_received () const
{
	auto value = rsnano::rsn_channel_get_last_packet_received (handle);
	return std::chrono::steady_clock::time_point (std::chrono::steady_clock::duration (value));
}

void nano::transport::channel::set_last_packet_sent (std::chrono::steady_clock::time_point const time_a)
{
	rsnano::rsn_channel_set_last_packet_sent (handle, time_a.time_since_epoch ().count ());
}

std::chrono::steady_clock::time_point nano::transport::channel::get_last_packet_sent () const
{
	auto value = rsnano::rsn_channel_get_last_packet_sent (handle);
	return std::chrono::steady_clock::time_point (std::chrono::steady_clock::duration (value));
}

void nano::transport::channel::set_last_packet_received (std::chrono::steady_clock::time_point const time_a)
{
	rsnano::rsn_channel_set_last_packet_received (handle, time_a.time_since_epoch ().count ());
}

boost::optional<nano::account> nano::transport::channel::get_node_id_optional () const
{
	nano::account result;
	if (rsnano::rsn_channel_get_node_id (handle, result.bytes.data ()))
	{
		return result;
	}

	return boost::none;
}

nano::account nano::transport::channel::get_node_id () const
{
	auto node_id{ get_node_id_optional () };
	nano::account result;
	if (node_id.is_initialized ())
	{
		result = node_id.get ();
	}
	else
	{
		result = 0;
	}
	return result;
}

void nano::transport::channel::set_node_id (nano::account node_id_a)
{
	rsnano::rsn_channel_set_node_id (handle, node_id_a.bytes.data ());
}