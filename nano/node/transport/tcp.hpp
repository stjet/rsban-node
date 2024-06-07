#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/election.hpp"
#include "nano/node/nodeconfig.hpp"

#include <nano/node/common.hpp>
#include <nano/node/peer_exclusion.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <memory>

namespace nano
{
class bootstrap_server;
class node_config;
class node_flags;
class network;
class syn_cookies;
class logger;

class tcp_message_manager final
{
public:
	explicit tcp_message_manager (rsnano::TcpMessageManagerHandle * handle);
	explicit tcp_message_manager (unsigned incoming_connections_max_a);
	tcp_message_manager (tcp_message_manager const &) = delete;
	tcp_message_manager (tcp_message_manager &&) = delete;
	~tcp_message_manager ();
	rsnano::TcpMessageManagerHandle * handle;
};

namespace transport
{
	class tcp_server;
	class tcp_channels;
	class tcp_listener;

	class request_response_visitor_factory
	{
	public:
		explicit request_response_visitor_factory (nano::node & node_a);
		request_response_visitor_factory (request_response_visitor_factory const &) = delete;
		~request_response_visitor_factory ();
		rsnano::RequestResponseVisitorFactoryHandle * handle;
	};

	void channel_tcp_send_callback (void * context_a, const rsnano::ErrorCodeDto * ec_a, std::size_t size_a);
	void delete_send_buffer_callback (void * context_a);

	class channel_tcp : public nano::transport::channel
	{
		friend class nano::transport::tcp_channels;

	public:
		channel_tcp (
		rsnano::async_runtime & async_rt_a,
		nano::outbound_bandwidth_limiter & limiter_a,
		nano::network_constants const & network_a,
		std::shared_ptr<nano::transport::socket> const & socket_a,
		nano::stats const & stats_a,
		nano::transport::tcp_channels const & tcp_channels_a,
		size_t channel_id);

		channel_tcp (rsnano::ChannelHandle * handle_a) :
			channel{ handle_a } {};

		uint8_t get_network_version () const override;
		void send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr, nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter, nano::transport::traffic_type = nano::transport::traffic_type::generic) override;
		size_t socket_id () const;

		std::string to_string () const override;

		nano::endpoint get_remote_endpoint () const override
		{
			return nano::transport::map_tcp_to_endpoint (get_tcp_remote_endpoint ());
		}

		nano::tcp_endpoint get_tcp_remote_endpoint () const override;
		nano::tcp_endpoint get_local_endpoint () const override;
		nano::transport::transport_type get_type () const override
		{
			return nano::transport::transport_type::tcp;
		}

		bool alive () const override;
	};

	class tcp_channels final : public std::enable_shared_from_this<tcp_channels>
	{
		friend class nano::transport::channel_tcp;

	public:
		explicit tcp_channels (nano::node &, uint16_t port);
		explicit tcp_channels (rsnano::TcpChannelsHandle * handle, rsnano::TcpMessageManagerHandle * mgr_handle, rsnano::NetworkFilterHandle * filter_handle);
		tcp_channels (nano::transport::tcp_channels const &) = delete;
		~tcp_channels ();

		std::size_t size () const;
		float size_sqrt () const;
		// Desired fanout for a given scale
		std::size_t fanout (float scale = 1.0f) const;
		std::shared_ptr<nano::transport::channel_tcp> find_channel (nano::tcp_endpoint const &) const;
		std::vector<std::shared_ptr<nano::transport::channel>> random_channels (std::size_t, uint8_t = 0) const;
		std::shared_ptr<nano::transport::channel_tcp> find_node_id (nano::account const &);
		bool not_a_peer (nano::endpoint const &, bool);
		void merge_peer (nano::endpoint const & peer_a);
		// Should we reach out to this endpoint with a keepalive message
		bool track_reachout (nano::endpoint const &);
		void purge (std::chrono::system_clock::time_point const & cutoff_deadline);
		std::deque<std::shared_ptr<nano::transport::channel>> list (std::size_t max_count = 0, uint8_t = 0);
		std::deque<std::shared_ptr<nano::transport::channel>> random_fanout (float scale = 1.0f);
		void flood_message (nano::message & msg, float scale);
		// Connection start
		void start_tcp (nano::endpoint const &);

		void random_fill (std::array<nano::endpoint, 8> &) const;
		uint16_t port () const;
		std::size_t get_next_channel_id ();

		nano::tcp_message_manager tcp_message_manager;
		nano::peer_exclusion excluded_peers ();
		std::shared_ptr<nano::network_filter> publish_filter;

	public:
		rsnano::TcpChannelsHandle * handle;

		friend class network_peer_max_tcp_attempts_subnetwork_Test;
	};

	std::shared_ptr<nano::transport::channel> channel_handle_to_channel (rsnano::ChannelHandle * handle);
} // namespace transport
} // namespace nano
