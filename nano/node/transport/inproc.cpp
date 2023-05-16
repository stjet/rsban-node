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
#include <memory>

namespace
{
rsnano::ChannelHandle * create_inproc_handle (size_t channel_id, nano::network_filter & network_filter, nano::network_constants & network_constants)
{
	auto network_dto{ network_constants.to_dto () };
	return rsnano::rsn_channel_inproc_create (
	channel_id,
	std::chrono::steady_clock::now ().time_since_epoch ().count (),
	&network_dto,
	network_filter.handle);
}
}

nano::transport::inproc::channel::channel (nano::node & node_a, nano::node & destination) :
	channel (
	node_a.network->next_channel_id.fetch_add (1),
	*node_a.network->publish_filter,
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
	transport::channel{ create_inproc_handle (channel_id, publish_filter, network) },
	stats (stats),
	limiter (outbound_limiter),
	io_ctx (io_ctx),
	endpoint{ endpoint },
	destination{ destination },
	network{ network },
	source_inbound{ source_inbound },
	destination_inbound{ destination_inbound },
	source_node_id{ source_node_id },
	destination_node_id{ destination_node_id }
{
	set_node_id (source_node_id);
	set_network_version (network.protocol_version);
}

std::size_t nano::transport::inproc::channel::hash_code () const
{
	std::hash<::nano::endpoint> hash;
	return hash (endpoint);
}

bool nano::transport::inproc::channel::operator== (nano::transport::channel const & other_a) const
{
	return endpoint == other_a.get_endpoint ();
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
	auto buffer (message_a.to_shared_const_buffer ());
	auto detail = nano::to_stat_detail (message_a.get_header ().get_type ());
	auto is_droppable_by_limiter = drop_policy_a == nano::transport::buffer_drop_policy::limiter;
	auto should_pass (limiter.should_pass (buffer.size (), nano::to_bandwidth_limit_type (traffic_type)));
	if (!is_droppable_by_limiter || should_pass)
	{
		send_buffer (buffer, callback_a, drop_policy_a);
		stats.inc (nano::stat::type::message, detail, nano::stat::dir::out);
	}
	else
	{
		if (callback_a)
		{
			io_ctx.post ([callback_a] () {
				callback_a (boost::system::errc::make_error_code (boost::system::errc::not_supported), 0);
			});
		}

		stats.inc (nano::stat::type::drop, detail, nano::stat::dir::out);
	}
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
	auto context = new std::function<void (boost::system::error_code, std::unique_ptr<nano::message>)> (
	[this] (boost::system::error_code ec_a, std::unique_ptr<nano::message> message_a) {
		if (ec_a || !message_a)
		{
			return;
		}
		nano::network_filter filter{ 100000 };
		// we create a temporary channel for the reply path, in case the receiver of the message wants to reply
		// auto remote_channel = std::make_shared<nano::transport::inproc::channel> (destination, node);
		auto remote_channel = std::make_shared<nano::transport::inproc::channel> (
		1,
		filter,
		network,
		stats,
		limiter,
		io_ctx,
		destination,
		destination_node_id,
		destination_inbound,
		endpoint,
		source_node_id,
		source_inbound);

		// process message
		{
			stats.inc (nano::stat::type::message, nano::to_stat_detail (message_a->get_header ().get_type ()), nano::stat::dir::in);

			// create an inbound message visitor class to handle incoming messages
			message_visitor_inbound visitor{ destination_inbound, remote_channel };
			message_a->visit (visitor);
		}
	});

	rsnano::rsn_channel_inproc_send_buffer (handle, buffer_a.data (), buffer_a.size (), message_received_callback, context, delete_callback_context);

	if (callback_a)
	{
		io_ctx.post ([callback_l = std::move (callback_a), buffer_size = buffer_a.size ()] () {
			callback_l (boost::system::errc::make_error_code (boost::system::errc::success), buffer_size);
		});
	}
}

std::string nano::transport::inproc::channel::to_string () const
{
	return boost::str (boost::format ("%1%") % endpoint);
}

void nano::transport::inproc::channel::set_peering_endpoint (nano::endpoint endpoint)
{
	peering_endpoint = endpoint;
}

nano::endpoint nano::transport::inproc::channel::get_peering_endpoint () const
{
	if (peering_endpoint)
	{
		return *peering_endpoint;
	}
	else
	{
		return get_endpoint ();
	}
}