#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/bandwidth_limiter.hpp"
#include "nano/node/transport/channel.hpp"

#include <nano/node/node.hpp>
#include <nano/node/transport/fake.hpp>

#include <boost/format.hpp>

namespace
{
rsnano::ChannelHandle * create_fake_channel (nano::node & node)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (node.network->endpoint ()) };
	auto network_dto{ node.network_params.network.to_dto () };
	return rsnano::rsn_channel_fake_create (
	node.network->tcp_channels->get_next_channel_id (),
	node.async_rt.handle,
	node.outbound_limiter.handle,
	node.stats->handle,
	&endpoint_dto,
	&network_dto);
}
}

nano::transport::fake::channel::channel (nano::node & node) :
	transport::channel{ create_fake_channel (node) }
{
	set_node_id (node.node_id.pub);
}

nano::transport::fake::channel::channel (rsnano::ChannelHandle * handle) :
	nano::transport::channel (handle)
{
}

std::string nano::transport::fake::channel::to_string () const
{
	return boost::str (boost::format ("%1%") % get_remote_endpoint ());
}

nano::endpoint nano::transport::fake::channel::get_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_fake_endpoint (handle, &dto);
	return rsnano::dto_to_udp_endpoint (dto);
}
