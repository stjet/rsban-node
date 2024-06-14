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

std::chrono::system_clock::time_point nano::transport::channel::get_last_packet_received () const
{
	return rsnano::time_point_from_nanoseconds (rsnano::rsn_channel_get_last_packet_received (handle));
}

nano::tcp_endpoint nano::transport::channel::get_peering_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_peering_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

void nano::transport::channel::set_last_packet_sent (std::chrono::system_clock::time_point time)
{
	rsnano::rsn_channel_set_last_packet_sent2 (handle, time.time_since_epoch ().count ());
}

std::chrono::system_clock::time_point nano::transport::channel::get_last_packet_sent () const
{
	return rsnano::time_point_from_nanoseconds (rsnano::rsn_channel_get_last_packet_sent (handle));
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

