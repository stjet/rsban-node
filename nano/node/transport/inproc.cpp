#include "boost/libs/asio/include/boost/asio/io_context.hpp"
#include "nano/lib/config.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/lib/stats.hpp"
#include "nano/node/bandwidth_limiter.hpp"
#include "nano/node/common.hpp"
#include "nano/node/messages.hpp"
#include "nano/node/transport/channel.hpp"
#include "nano/node/transport/traffic_type.hpp"
#include "nano/secure/network_filter.hpp"

#include <nano/node/node.hpp>
#include <nano/node/transport/inproc.hpp>

#include <boost/format.hpp>

#include <cstddef>
#include <ios>
#include <memory>
#include <stdexcept>

void nano::transport::delete_inbound_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	delete callback;
}

void nano::transport::inbound_wrapper (void * context, rsnano::MessageHandle * message_handle, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	auto message = rsnano::message_handle_to_message (message_handle);
	std::shared_ptr<nano::transport::channel> channel{ std::make_shared<nano::transport::inproc::channel> (channel_handle) };
	(*callback) (*message, channel);
}

namespace
{
rsnano::ChannelHandle * create_inproc_handle (
size_t channel_id,
nano::network_filter & network_filter,
nano::network_constants & network_constants,
nano::stats & stats,
nano::outbound_bandwidth_limiter & outbound_limiter,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> source_inbound,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> destination_inbound,
rsnano::async_runtime & async_rt,
nano::endpoint source,
nano::endpoint destination,
nano::account source_node_id,
nano::account destination_node_id)
{
	auto source_context = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> (source_inbound);
	auto destination_context = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> (destination_inbound);
	auto network_dto{ network_constants.to_dto () };
	auto source_dto = rsnano::udp_endpoint_to_dto (source);
	auto destination_dto = rsnano::udp_endpoint_to_dto (destination);

	return rsnano::rsn_channel_inproc_create (
	channel_id,
	&network_dto,
	network_filter.handle,
	stats.handle,
	outbound_limiter.handle,
	nano::transport::inbound_wrapper,
	source_context,
	nano::transport::inbound_wrapper,
	destination_context,
	nano::transport::delete_inbound_context,
	async_rt.handle,
	&source_dto,
	&destination_dto,
	source_node_id.bytes.data (),
	destination_node_id.bytes.data ());
}
}

nano::transport::inproc::channel::channel (nano::node & node_a, nano::node & destination) :
	channel (
	node_a.network->tcp_channels->get_next_channel_id (),
	*node_a.network->tcp_channels->publish_filter,
	node_a.config->network_params.network,
	*node_a.stats,
	node_a.outbound_limiter,
	node_a.async_rt,
	node_a.network->endpoint (),
	node_a.node_id.pub,
	std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> ([&node_a] (nano::message const & msg, std::shared_ptr<nano::transport::channel> const & channel) { node_a.network->inbound (msg, channel); }),
	destination.network->endpoint (),
	destination.node_id.pub,
	std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> ([&destination] (nano::message const & msg, std::shared_ptr<nano::transport::channel> const & channel) { destination.network->inbound (msg, channel); }))
{
}

nano::transport::inproc::channel::channel (rsnano::ChannelHandle * handle_a) :
	nano::transport::channel (handle_a)
{
}

nano::transport::inproc::channel::channel (
size_t channel_id,
nano::network_filter & publish_filter,
nano::network_constants & network,
nano::stats & stats,
nano::outbound_bandwidth_limiter & outbound_limiter,
rsnano::async_runtime & async_rt,
nano::endpoint endpoint,
nano::account source_node_id,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> source_inbound,
nano::endpoint destination,
nano::account destination_node_id,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> destination_inbound) :
	transport::channel{ create_inproc_handle (
	channel_id,
	publish_filter,
	network,
	stats,
	outbound_limiter,
	source_inbound,
	destination_inbound,
	async_rt,
	endpoint,
	destination,
	source_node_id,
	destination_node_id) }
{
}

uint8_t nano::transport::inproc::channel::get_network_version () const
{
	return rsnano::rsn_channel_inproc_network_version (handle);
}

nano::endpoint nano::transport::inproc::channel::get_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_inproc_endpoint (handle, &dto);
	return rsnano::dto_to_udp_endpoint (dto);
}

nano::tcp_endpoint nano::transport::inproc::channel::get_tcp_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_inproc_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

std::string nano::transport::inproc::channel::to_string () const
{
	return boost::str (boost::format ("%1%") % get_remote_endpoint ());
}
