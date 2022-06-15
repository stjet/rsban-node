#pragma once

#include <nano/node/transport/transport.hpp>

namespace nano
{
class node;

namespace transport
{
	/**
	 * In-process transport channel. Mostly useful for unit tests
	**/
	namespace inproc
	{
		class channel final : public nano::transport::channel
		{
		public:
			explicit channel (nano::node & node, nano::node & destination);

			std::chrono::steady_clock::time_point get_last_packet_received () const override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				return last_packet_received;
			}

			void set_last_packet_received (std::chrono::steady_clock::time_point const time_a) override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				last_packet_received = time_a;
			}

			std::chrono::steady_clock::time_point get_last_packet_sent () const override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				return last_packet_sent;
			}

			void set_last_packet_sent (std::chrono::steady_clock::time_point const time_a) override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				last_packet_sent = time_a;
			}

			boost::optional<nano::account> get_node_id_optional () const override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				return node_id;
			}

			nano::account get_node_id () const override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				if (node_id.is_initialized ())
				{
					return node_id.get ();
				}
				else
				{
					return 0;
				}
			}

			void set_node_id (nano::account node_id_a) override
			{
				nano::lock_guard<nano::mutex> lk (channel_mutex);
				node_id = node_id_a;
			}

			uint8_t get_network_version () const override
			{
				return network_version;
			}

			void set_network_version (uint8_t network_version_a) override
			{
				network_version = network_version_a;
			}

			std::size_t hash_code () const override;
			bool operator== (nano::transport::channel const &) const override;
			// TODO: investigate clang-tidy warning about default parameters on virtual/override functions
			//
			void send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr, nano::buffer_drop_policy policy_a = nano::buffer_drop_policy::limiter) override;
			void send_buffer (nano::shared_const_buffer const &, std::function<void (boost::system::error_code const &, std::size_t)> const & = nullptr, nano::buffer_drop_policy = nano::buffer_drop_policy::limiter) override;
			std::string to_string () const override;
			bool operator== (nano::transport::inproc::channel const & other_a) const
			{
				return endpoint == other_a.get_endpoint ();
			}

			nano::endpoint get_endpoint () const override
			{
				return endpoint;
			}

			nano::tcp_endpoint get_tcp_endpoint () const override
			{
				return nano::transport::map_endpoint_to_tcp (endpoint);
			}

			nano::transport::transport_type get_type () const override
			{
				return nano::transport::transport_type::loopback;
			}

		private:
			boost::asio::io_context & io_ctx;
			nano::stat & stats;
			nano::logger_mt & logger;
			nano::bandwidth_limiter & limiter;
			bool network_packet_logging;
			mutable nano::mutex channel_mutex;
			std::chrono::steady_clock::time_point last_packet_received{ std::chrono::steady_clock::now () };
			std::chrono::steady_clock::time_point last_packet_sent{ std::chrono::steady_clock::now () };
			boost::optional<nano::account> node_id{ boost::none };
			std::atomic<uint8_t> network_version{ 0 };
			nano::node & node;
			nano::node & destination;
			nano::endpoint const endpoint;
		};
	} // namespace inproc
} // namespace transport
} // namespace nano
