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

namespace
{
void delete_inbound_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	delete callback;
}

void inbound_wrapper (void * context, rsnano::MessageHandle * message_handle, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	auto message = rsnano::message_handle_to_message (message_handle);
	std::shared_ptr<nano::transport::channel> channel{ std::make_shared<nano::transport::inproc::channel> (channel_handle) };
	(*callback) (*message, channel);
}

rsnano::ChannelHandle * create_inproc_handle (
size_t channel_id,
nano::network_filter & network_filter,
nano::network_constants & network_constants,
nano::stats & stats,
nano::outbound_bandwidth_limiter & outbound_limiter,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> source_inbound,
std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> destination_inbound,
boost::asio::io_context & io_ctx,
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
	inbound_wrapper,
	source_context,
	inbound_wrapper,
	destination_context,
	delete_inbound_context,
	&io_ctx,
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
	node_a.io_ctx,
	node_a.network->endpoint (),
	node_a.node_id.pub,
	node_a.network->inbound,
	destination.network->endpoint (),
	destination.node_id.pub,
	destination.network->inbound)
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
boost::asio::io_context & io_ctx,
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
	io_ctx,
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

nano::endpoint nano::transport::inproc::channel::get_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_inproc_endpoint (handle, &dto);
	return rsnano::dto_to_udp_endpoint (dto);
}

nano::tcp_endpoint nano::transport::inproc::channel::get_tcp_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_inproc_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

std::size_t nano::transport::inproc::channel::hash_code () const
{
	std::hash<::nano::endpoint> hash;
	return hash (get_endpoint ());
}

bool nano::transport::inproc::channel::operator== (nano::transport::channel const & other_a) const
{
	return get_endpoint () == other_a.get_endpoint ();
}

/**
 *  This function is called for every message received by the inproc channel.
 *  Note that it is called from inside the context of nano::transport::inproc::channel::send_buffer
 */
class message_visitor_inbound : public nano::message_visitor
{
public:
	message_visitor_inbound (decltype (nano::network::inbound) & inbound, std::shared_ptr<nano::transport::inproc::channel> channel) :
		inbound{ inbound },
		channel{ channel }
	{
	}

	decltype (nano::network::inbound) & inbound;

	// the channel to reply to, if a reply is generated
	std::shared_ptr<nano::transport::inproc::channel> channel;

	void default_handler (nano::message const & message) override
	{
		inbound (message, channel);
	}
};

void nano::transport::inproc::channel::send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_inproc_send (handle, message_a.handle, nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

namespace
{
void message_received_callback (void * context, const rsnano::ErrorCodeDto * ec_dto, rsnano::MessageHandle * msg_handle)
{
	auto callback = static_cast<std::function<void (boost::system::error_code, std::unique_ptr<nano::message>)> *> (context);
	auto ec = rsnano::dto_to_error_code (*ec_dto);
	std::unique_ptr<nano::message> message;
	if (msg_handle != nullptr)
	{
		message = rsnano::message_handle_to_message (rsnano::rsn_message_clone (msg_handle));
	}
	(*callback) (ec, std::move (message));
}

void delete_callback_context (void * context)
{
	auto callback = static_cast<std::function<void (boost::system::error_code, std::unique_ptr<nano::message>)> *> (context);
	delete callback;
}
}

/**
 * Send the buffer to the peer and call the callback function when done. The call never fails.
 * Note that the inbound message visitor will be called before the callback because it is called directly whereas the callback is spawned in the background.
 */
void nano::transport::inproc::channel::send_buffer (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_inproc_send_buffer (handle, buffer_a.data (), buffer_a.size (), nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

std::string nano::transport::inproc::channel::to_string () const
{
	return boost::str (boost::format ("%1%") % get_endpoint ());
}

void nano::transport::inproc::channel::set_peering_endpoint (nano::endpoint endpoint)
{
	throw std::runtime_error ("set_peering_endpoint not yet implemented for inproc channel");
}

nano::endpoint nano::transport::inproc::channel::get_peering_endpoint () const
{
	return get_endpoint ();
}