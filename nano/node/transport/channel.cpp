#include "nano/node/transport/tcp.hpp"

#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <boost/asio/ip/address.hpp>
#include <boost/asio/ip/address_v4.hpp>
#include <boost/asio/ip/address_v6.hpp>
#include <boost/format.hpp>

#include <chrono>

nano::transport::channel::channel (rsnano::ChannelHandle * handle_a) :
	handle (handle_a)
{
}

nano::transport::channel::~channel ()
{
	rsnano::rsn_channel_destroy (handle);
}

void nano::transport::channel::close ()
{
	rsnano::rsn_channel_close (handle);
}

std::chrono::system_clock::time_point nano::transport::channel::get_last_bootstrap_attempt () const
{
	return rsnano::time_point_from_nanoseconds (rsnano::rsn_channel_get_last_bootstrap_attempt (handle));
}

void nano::transport::channel::set_last_bootstrap_attempt ()
{
	rsnano::rsn_channel_set_last_bootstrap_attempt (handle);
}

std::chrono::system_clock::time_point nano::transport::channel::get_last_packet_received () const
{
	return rsnano::time_point_from_nanoseconds (rsnano::rsn_channel_get_last_packet_received (handle));
}

void nano::transport::channel::set_last_packet_sent ()
{
	rsnano::rsn_channel_set_last_packet_sent (handle);
}

void nano::transport::channel::set_last_packet_sent (std::chrono::system_clock::time_point time)
{
	rsnano::rsn_channel_set_last_packet_sent2 (handle, time.time_since_epoch ().count ());
}

std::chrono::system_clock::time_point nano::transport::channel::get_last_packet_sent () const
{
	return rsnano::time_point_from_nanoseconds (rsnano::rsn_channel_get_last_packet_sent (handle));
}

void nano::transport::channel::set_last_packet_received ()
{
	rsnano::rsn_channel_set_last_packet_received (handle);
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

size_t nano::transport::channel::channel_id () const
{
	return rsnano::rsn_channel_id (handle);
}

nano::transport::channel_weak_ptr::channel_weak_ptr (const std::shared_ptr<nano::transport::channel> & channel_a) :
	handle{ rsnano::rsn_channel_to_weak (channel_a->handle) }
{
}

nano::transport::channel_weak_ptr::channel_weak_ptr (channel_weak_ptr && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}
nano::transport::channel_weak_ptr::~channel_weak_ptr ()
{
	if (handle)
	{
		rsnano::rsn_channel_weak_destroy (handle);
	}
}

std::shared_ptr<nano::transport::channel> nano::transport::channel_weak_ptr::upgrade () const
{
	auto channel_handle = rsnano::rsn_channel_weak_upgrade (handle);
	std::shared_ptr<nano::transport::channel> result;
	if (channel_handle)
	{
		result = nano::transport::channel_handle_to_channel (channel_handle);
	}
	return result;
}
