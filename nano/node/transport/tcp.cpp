#include "boost/asio/io_context.hpp"
#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/nodeconfig.hpp"
#include "nano/node/peer_exclusion.hpp"
#include "nano/node/transport/channel.hpp"
#include "nano/node/transport/socket.hpp"
#include "nano/node/transport/tcp_server.hpp"
#include "nano/node/transport/traffic_type.hpp"
#include "nano/secure/common.hpp"
#include "nano/secure/network_filter.hpp"

#include <nano/lib/config.hpp>
#include <nano/lib/stats.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/fake.hpp>
#include <nano/node/transport/inproc.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <stdexcept>
#include <unordered_set>

/*
 * tcp_message_manager
 */

nano::tcp_message_manager::tcp_message_manager (unsigned incoming_connections_max_a) :
	handle{ rsnano::rsn_tcp_message_manager_create (incoming_connections_max_a) }
{
}

nano::tcp_message_manager::~tcp_message_manager ()
{
	rsnano::rsn_tcp_message_manager_destroy (handle);
}

void nano::tcp_message_manager::put_message (nano::tcp_message_item const & item_a)
{
	rsnano::rsn_tcp_message_manager_put_message (handle, item_a.handle);
}

nano::tcp_message_item nano::tcp_message_manager::get_message ()
{
	return nano::tcp_message_item{ rsnano::rsn_tcp_message_manager_get_message (handle) };
}

void nano::tcp_message_manager::stop ()
{
	rsnano::rsn_tcp_message_manager_stop (handle);
}

/*
 * channel_tcp
 */

nano::transport::channel_tcp::channel_tcp (
boost::asio::io_context & io_ctx_a,
nano::outbound_bandwidth_limiter & limiter_a,
nano::network_constants const & network_a,
std::shared_ptr<nano::transport::socket> const & socket_a,
std::shared_ptr<nano::transport::channel_tcp_observer> const & observer_a,
size_t channel_id) :
	channel (rsnano::rsn_channel_tcp_create (
	socket_a->handle,
	new std::weak_ptr<nano::transport::channel_tcp_observer> (observer_a),
	limiter_a.handle,
	&io_ctx_a,
	channel_id))
{
	set_network_version (network_a.protocol_version);
}

uint8_t nano::transport::channel_tcp::get_network_version () const
{
	return rsnano::rsn_channel_tcp_network_version (handle);
}

void nano::transport::channel_tcp::set_network_version (uint8_t network_version_a)
{
	rsnano::rsn_channel_tcp_network_set_version (handle, network_version_a);
}

nano::tcp_endpoint nano::transport::channel_tcp::get_tcp_endpoint () const
{
	rsnano::EndpointDto ep_dto{};
	rsnano::rsn_channel_tcp_endpoint (handle, &ep_dto);
	return rsnano::dto_to_endpoint (ep_dto);
}

bool nano::transport::channel_tcp::max (nano::transport::traffic_type traffic_type)
{
	return rsnano::rsn_channel_tcp_max (handle, static_cast<uint8_t> (traffic_type));
}

std::size_t nano::transport::channel_tcp::hash_code () const
{
	std::hash<::nano::tcp_endpoint> hash;
	return hash (get_tcp_endpoint ());
}

bool nano::transport::channel_tcp::operator== (nano::transport::channel const & other_a) const
{
	bool result (false);
	auto other_l (dynamic_cast<nano::transport::channel_tcp const *> (&other_a));
	if (other_l != nullptr)
	{
		return *this == *other_l;
	}
	return result;
}

void nano::transport::channel_tcp_send_callback (void * context_a, const rsnano::ErrorCodeDto * ec_a, std::size_t size_a)
{
	auto callback_ptr = static_cast<std::function<void (boost::system::error_code const &, std::size_t)> *> (context_a);
	if (*callback_ptr)
	{
		auto ec{ rsnano::dto_to_error_code (*ec_a) };
		(*callback_ptr) (ec, size_a);
	}
}

void nano::transport::delete_send_buffer_callback (void * context_a)
{
	auto callback_ptr = static_cast<std::function<void (boost::system::error_code const &, std::size_t)> *> (context_a);
	delete callback_ptr;
}

void nano::transport::channel_tcp::send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy drop_policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_tcp_send (handle, message_a.handle, nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (drop_policy_a), static_cast<uint8_t> (traffic_type));
}

void nano::transport::channel_tcp::send_buffer (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a, nano::transport::buffer_drop_policy policy_a, nano::transport::traffic_type traffic_type)
{
	auto callback_pointer = new std::function<void (boost::system::error_code const &, std::size_t)> (callback_a);
	rsnano::rsn_channel_tcp_send_buffer (handle, buffer_a.data (), buffer_a.size (), nano::transport::channel_tcp_send_callback, nano::transport::delete_send_buffer_callback, callback_pointer, static_cast<uint8_t> (policy_a), static_cast<uint8_t> (traffic_type));
}

std::string nano::transport::channel_tcp::to_string () const
{
	return boost::str (boost::format ("%1%") % get_tcp_endpoint ());
}

bool nano::transport::channel_tcp::operator== (nano::transport::channel_tcp const & other_a) const
{
	return rsnano::rsn_channel_tcp_eq (handle, other_a.handle);
}

std::shared_ptr<nano::transport::socket> nano::transport::channel_tcp::try_get_socket () const
{
	auto socket_handle{ rsnano::rsn_channel_tcp_socket (handle) };
	std::shared_ptr<nano::transport::socket> socket;
	if (socket_handle)
	{
		socket = std::make_shared<nano::transport::socket> (socket_handle);
	}
	return socket;
}

void nano::transport::channel_tcp::set_endpoint ()
{
	rsnano::rsn_channel_tcp_set_endpoint (handle);
}

nano::endpoint nano::transport::channel_tcp::get_peering_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_channel_tcp_peering_endpoint (handle, &dto);
	return rsnano::dto_to_udp_endpoint (dto);
}

void nano::transport::channel_tcp::set_peering_endpoint (nano::endpoint endpoint)
{
	auto dto{ rsnano::udp_endpoint_to_dto (endpoint) };
	rsnano::rsn_channel_tcp_set_peering_endpoint (handle, &dto);
}

bool nano::transport::channel_tcp::alive () const
{
	return rsnano::rsn_channel_tcp_is_alive (handle);
}

/*
 * tcp_channels
 */

namespace
{
void sink_callback (void * callback_handle, rsnano::MessageHandle * msg_handle, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (callback_handle);
	auto channel = std::make_shared<nano::transport::channel_tcp> (channel_handle);
	auto message = rsnano::message_handle_to_message (msg_handle);
	(*callback) (*message, channel);
}

void delete_sink (void * callback_handle)
{
	auto callback = static_cast<std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> *> (callback_handle);
	delete callback;
}
}

nano::transport::tcp_channels::tcp_channels (nano::node & node, uint16_t port, std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> sink) :
	tcp_message_manager{ node.config->tcp_incoming_connections_max },
	stats{ node.stats },
	config{ node.config },
	logger{ node.logger },
	publish_filter{ std::make_shared<nano::network_filter> (256 * 1024) }
{
	auto node_config_dto{ node.config->to_dto () };
	auto network_dto{ node.config->network_params.to_dto () };
	rsnano::io_ctx_wrapper io_ctx{ node.io_ctx };
	rsnano::TcpChannelsOptionsDto options;
	options.node_config = &node_config_dto;
	options.logger = nano::to_logger_handle (node.logger);
	options.publish_filter = publish_filter->handle;
	options.io_ctx = io_ctx.handle ();
	options.network = &network_dto;
	options.stats = node.stats->handle;
	options.block_uniquer = node.block_uniquer.handle;
	options.vote_uniquer = node.vote_uniquer.handle;
	options.tcp_message_manager = tcp_message_manager.handle;
	options.port = port;
	options.flags = node.flags.handle;
	options.sink_handle = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> (sink);
	options.sink_callback = sink_callback;
	options.delete_sink = delete_sink;
	options.limiter = node.outbound_limiter.handle;
	options.node_id_prv = node.node_id.prv.bytes.data ();
	options.syn_cookies = node.network->syn_cookies->handle;
	options.workers = node.workers->handle;
	options.tcp_socket_factory = new std::shared_ptr<nano::transport::tcp_socket_facade_factory> (std::make_shared<nano::transport::tcp_socket_facade_factory> (node.io_ctx));
	options.socket_observer = new std::weak_ptr<nano::node_observers> (node.observers);

	handle = rsnano::rsn_tcp_channels_create (&options);
}

nano::transport::tcp_channels::~tcp_channels ()
{
	rsnano::rsn_tcp_channels_destroy (handle);
}

void nano::transport::tcp_channels::erase (nano::tcp_endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	rsnano::rsn_tcp_channels_erase_channel_by_endpoint (handle, &endpoint_dto);
}

void nano::transport::tcp_channels::erase_temporary_channel (nano::tcp_endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	rsnano::rsn_tcp_channels_erase_temporary_channel (handle, &endpoint_dto);
}

std::size_t nano::transport::tcp_channels::size () const
{
	return rsnano::rsn_tcp_channels_channel_count (handle);
}

std::shared_ptr<nano::transport::channel_tcp> nano::transport::tcp_channels::find_channel (nano::tcp_endpoint const & endpoint_a) const
{
	std::shared_ptr<nano::transport::channel_tcp> result;
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	auto channel_handle = rsnano::rsn_tcp_channels_find_channel (handle, &endpoint_dto);
	if (channel_handle)
	{
		result = std::make_shared<nano::transport::channel_tcp> (channel_handle);
	}
	return result;
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::transport::tcp_channels::random_channels (std::size_t count_a, uint8_t min_version, bool include_temporary_channels_a) const
{
	auto list_handle = rsnano::rsn_tcp_channels_random_channels (handle, count_a, min_version, include_temporary_channels_a);
	auto len = rsnano::rsn_channel_list_len (list_handle);
	std::vector<std::shared_ptr<nano::transport::channel>> result;
	result.reserve (len);
	for (auto i = 0; i < len; ++i)
	{
		auto channel_handle = rsnano::rsn_channel_list_get (list_handle, i);
		result.push_back (std::make_shared<nano::transport::channel_tcp> (channel_handle));
	}
	rsnano::rsn_channel_list_destroy (list_handle);
	return result;
}

std::vector<nano::endpoint> nano::transport::tcp_channels::get_peers () const
{
	auto list_handle = rsnano::rsn_tcp_channels_get_peers (handle);
	auto len = rsnano::rsn_endpoint_list_len (list_handle);
	std::vector<nano::endpoint> endpoints;
	endpoints.reserve (len);
	for (auto i = 0; i < len; ++i)
	{
		rsnano::EndpointDto dto;
		rsnano::rsn_endpoint_list_get (list_handle, i, &dto);
		endpoints.push_back (rsnano::dto_to_udp_endpoint (dto));
	}
	rsnano::rsn_endpoint_list_destroy (list_handle);
	return endpoints;
}

void nano::transport::tcp_channels::random_fill (std::array<nano::endpoint, 8> & target_a) const
{
	std::array<rsnano::EndpointDto, 8> dtos;
	rsnano::rsn_tcp_channels_random_fill (handle, dtos.data ());
	auto j{ target_a.begin () };
	for (auto i{ dtos.begin () }, n{ dtos.end () }; i != n; ++i, ++j)
	{
		*j = rsnano::dto_to_udp_endpoint (*i);
	}
}

void nano::transport::tcp_channels::set_port (uint16_t port_a)
{
	rsnano::rsn_tcp_channels_set_port (handle, port_a);
}

void nano::transport::tcp_channels::set_observer (std::shared_ptr<nano::tcp_server_observer> observer_a)
{
	auto observer_handle = new std::weak_ptr<nano::tcp_server_observer> (observer_a);
	rsnano::rsn_tcp_channels_set_observer (handle, observer_handle);
}

void nano::transport::tcp_channels::set_message_visitor_factory (nano::transport::request_response_visitor_factory & visitor_factory)
{
	rsnano::rsn_tcp_channels_set_message_visitor (handle, visitor_factory.handle);
}

std::shared_ptr<nano::transport::channel_tcp> nano::transport::tcp_channels::get_first_channel () const
{
	return std::make_shared<nano::transport::channel_tcp> (rsnano::rsn_tcp_channels_get_first_channel (handle));
}

std::size_t nano::transport::tcp_channels::get_next_channel_id ()
{
	return rsnano::rsn_tcp_channels_get_next_channel_id (handle);
}

nano::peer_exclusion nano::transport::tcp_channels::excluded_peers ()
{
	return nano::peer_exclusion{ rsnano::rsn_tcp_channels_excluded_peers (handle) };
}

std::shared_ptr<nano::transport::channel_tcp> nano::transport::tcp_channels::find_node_id (nano::account const & node_id_a)
{
	std::shared_ptr<nano::transport::channel_tcp> result;
	auto channel_handle = rsnano::rsn_tcp_channels_find_node_id (handle, node_id_a.bytes.data ());
	if (channel_handle)
	{
		result = std::make_shared<nano::transport::channel_tcp> (channel_handle);
	}
	return result;
}

nano::tcp_endpoint nano::transport::tcp_channels::bootstrap_peer ()
{
	rsnano::EndpointDto endpoint_dto;
	rsnano::rsn_tcp_channels_bootstrap_peer (handle, &endpoint_dto);
	return rsnano::dto_to_endpoint (endpoint_dto);
}

void nano::transport::tcp_channels::process_messages ()
{
	rsnano::rsn_tcp_channels_process_messages (handle);
}

void nano::transport::tcp_channels::start ()
{
	ongoing_keepalive ();
}

void nano::transport::tcp_channels::stop ()
{
	rsnano::rsn_tcp_channels_stop (handle);
}

bool nano::transport::tcp_channels::not_a_peer (nano::endpoint const & endpoint_a, bool allow_local_peers)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	return rsnano::rsn_tcp_channels_not_a_peer (handle, &endpoint_dto, allow_local_peers);
}

bool nano::transport::tcp_channels::reachout (nano::endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	return rsnano::rsn_tcp_channels_reachout (handle, &endpoint_dto);
}

std::unique_ptr<nano::container_info_component> nano::transport::tcp_channels::collect_container_info (std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_tcp_channels_collect_container_info (handle, name.c_str ()));
}

void nano::transport::tcp_channels::purge (std::chrono::system_clock::time_point const & cutoff_a)
{
	uint64_t cutoff_ns = std::chrono::duration_cast<std::chrono::nanoseconds> (cutoff_a.time_since_epoch ()).count ();
	rsnano::rsn_tcp_channels_purge (handle, cutoff_ns);
}

void nano::transport::tcp_channels::ongoing_keepalive ()
{
	rsnano::rsn_tcp_channels_ongoing_keepalive (handle);
}

void nano::transport::tcp_channels::list (std::deque<std::shared_ptr<nano::transport::channel>> & deque_a, uint8_t minimum_version_a, bool include_temporary_channels_a)
{
	auto list_handle = rsnano::rsn_tcp_channels_list_channels (handle, minimum_version_a, include_temporary_channels_a);
	auto len = rsnano::rsn_channel_list_len (list_handle);
	for (auto i = 0; i < len; ++i)
	{
		auto channel{ std::make_shared<nano::transport::channel_tcp> (rsnano::rsn_channel_list_get (list_handle, i)) };
		deque_a.push_back (channel);
	}
	rsnano::rsn_channel_list_destroy (list_handle);
}

void nano::transport::tcp_channels::modify_last_packet_sent (nano::endpoint const & endpoint_a, std::chrono::system_clock::time_point const & time_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	auto time_ns = std::chrono::duration_cast<std::chrono::nanoseconds> (time_a.time_since_epoch ()).count ();
	rsnano::rsn_tcp_channels_set_last_packet_sent (handle, &endpoint_dto, time_ns);
}

void nano::transport::tcp_channels::update (nano::tcp_endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	rsnano::rsn_tcp_channels_update_channel (handle, &endpoint_dto);
}

void nano::transport::tcp_channels::start_tcp (nano::endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	rsnano::rsn_tcp_channels_start_tcp (handle, &endpoint_dto);
	return;
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

void nano::transport::tcp_channels::data_sent (boost::asio::ip::tcp::endpoint const & endpoint_a)
{
	update (endpoint_a);
}

void nano::transport::tcp_channels::host_unreachable ()
{
	stats->inc (nano::stat::type::error, nano::stat::detail::unreachable_host, nano::stat::dir::out);
}

void nano::transport::tcp_channels::message_sent (nano::message const & message_a)
{
	auto detail = nano::to_stat_detail (message_a.get_header ().get_type ());
	stats->inc (nano::stat::type::message, detail, nano::stat::dir::out);
}

void nano::transport::tcp_channels::message_dropped (nano::message const & message_a, std::size_t buffer_size_a)
{
	nano::transport::callback_visitor visitor;
	message_a.visit (visitor);
	stats->inc (nano::stat::type::drop, visitor.result, nano::stat::dir::out);
	if (config->logging.network_packet_logging ())
	{
		logger->always_log (boost::str (boost::format ("%1% of size %2% dropped") % stats->detail_to_string (visitor.result) % buffer_size_a));
	}
}

void nano::transport::tcp_channels::no_socket_drop ()
{
	stats->inc (nano::stat::type::tcp, nano::stat::detail::tcp_write_no_socket_drop, nano::stat::dir::out);
}

void nano::transport::tcp_channels::write_drop ()
{
	stats->inc (nano::stat::type::tcp, nano::stat::detail::tcp_write_drop, nano::stat::dir::out);
}

namespace
{
void delete_new_channel_callback (void * context)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::transport::channel>)> *> (context);
	delete callback;
}

void call_new_channel_callback (void * context, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::transport::channel>)> *> (context);
	auto channel = std::make_shared<nano::transport::channel_tcp> (channel_handle);
	(*callback) (channel);
}
}

void nano::transport::tcp_channels::on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a)
{
	auto callback_handle = new std::function<void (std::shared_ptr<nano::transport::channel>)> (observer_a);
	rsnano::rsn_tcp_channels_on_new_channel (handle, callback_handle, call_new_channel_callback, delete_new_channel_callback);
}

nano::tcp_message_item::tcp_message_item () :
	handle{ rsnano::rsn_tcp_message_item_empty () }
{
}

nano::tcp_message_item::~tcp_message_item ()
{
	if (handle)
		rsnano::rsn_tcp_message_item_destroy (handle);
}

nano::tcp_message_item::tcp_message_item (std::shared_ptr<nano::message> message_a, nano::tcp_endpoint endpoint_a, nano::account node_id_a, std::shared_ptr<nano::transport::socket> socket_a)
{
	rsnano::MessageHandle * message_handle = nullptr;
	if (message_a)
	{
		message_handle = message_a->handle;
	}

	rsnano::EndpointDto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	rsnano::SocketHandle * socket_handle = nullptr;
	if (socket_a)
	{
		socket_handle = socket_a->handle;
	}
	handle = rsnano::rsn_tcp_message_item_create (message_handle, &endpoint_dto, node_id_a.bytes.data (), socket_handle);
}

nano::tcp_message_item::tcp_message_item (nano::tcp_message_item const & other_a) :
	handle{ rsnano::rsn_tcp_message_item_clone (other_a.handle) }
{
}

nano::tcp_message_item::tcp_message_item (nano::tcp_message_item && other_a) noexcept :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::tcp_message_item::tcp_message_item (rsnano::TcpMessageItemHandle * handle_a) :
	handle{ handle_a }
{
}

std::shared_ptr<nano::message> nano::tcp_message_item::get_message () const
{
	auto message_handle = rsnano::rsn_tcp_message_item_message (handle);
	return nano::message_handle_to_message (message_handle);
}

nano::tcp_endpoint nano::tcp_message_item::get_endpoint () const
{
	rsnano::EndpointDto endpoint_dto;
	rsnano::rsn_tcp_message_item_endpoint (handle, &endpoint_dto);
	return rsnano::dto_to_endpoint (endpoint_dto);
}

nano::account nano::tcp_message_item::get_node_id () const
{
	nano::account node_id;
	rsnano::rsn_tcp_message_item_node_id (handle, node_id.bytes.data ());
	return node_id;
}

std::shared_ptr<nano::transport::socket> nano::tcp_message_item::get_socket () const
{
	auto socket_handle = rsnano::rsn_tcp_message_item_socket (handle);
	return std::make_shared<nano::transport::socket> (socket_handle);
}

nano::tcp_message_item & nano::tcp_message_item::operator= (tcp_message_item const & other_a)
{
	if (handle != nullptr)
		rsnano::rsn_tcp_message_item_destroy (handle);
	handle = rsnano::rsn_tcp_message_item_clone (other_a.handle);
	return *this;
}
nano::tcp_message_item & nano::tcp_message_item::operator= (tcp_message_item && other_a)
{
	if (handle != nullptr)
		rsnano::rsn_tcp_message_item_destroy (handle);
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}

std::shared_ptr<nano::transport::channel> nano::transport::channel_handle_to_channel (rsnano::ChannelHandle * handle)
{
	auto channel_type = static_cast<nano::transport::transport_type> (rsnano::rsn_channel_type (handle));
	switch (channel_type)
	{
		case nano::transport::transport_type::tcp:
			return make_shared<nano::transport::channel_tcp> (handle);
		case nano::transport::transport_type::loopback:
			return make_shared<nano::transport::inproc::channel> (handle);
		case nano::transport::transport_type::fake:
			return make_shared<nano::transport::fake::channel> (handle);
		default:
			throw std::runtime_error ("unknown transport type");
	}
}
