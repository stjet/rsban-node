#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

namespace nano
{
namespace transport
{
	/**
	 * Fake channel that connects to nothing and allows its attributes to be manipulated. Mostly useful for unit tests.
	 **/
	namespace fake
	{
		class channel final : public nano::transport::channel
		{
		public:
			explicit channel (nano::node &);
			explicit channel (rsnano::ChannelHandle * handle);

			std::string to_string () const override;
			std::size_t hash_code () const override;

			void send (nano::message & message_a,
			std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr,
			nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter,
			nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

			// clang-format off
			void send_buffer (
			nano::shared_const_buffer const &,
			std::function<void (boost::system::error_code const &, std::size_t)> const & = nullptr,
			nano::transport::buffer_drop_policy = nano::transport::buffer_drop_policy::limiter,
			nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

			bool operator== (nano::transport::channel const &) const override;
			bool operator== (nano::transport::fake::channel const & other_a) const;

			uint8_t get_network_version () const override
			{
				return rsnano::rsn_channel_fake_network_version (handle);
			}

			nano::endpoint get_endpoint () const override;

			nano::tcp_endpoint get_tcp_endpoint () const override
			{
				return nano::transport::map_endpoint_to_tcp (get_endpoint());
			}

			nano::transport::transport_type get_type () const override
			{
				return nano::transport::transport_type::fake;
			}

			nano::endpoint get_peering_endpoint () const override;
			void set_peering_endpoint (nano::endpoint endpoint) override;

			void close ()
			{
				rsnano::rsn_channel_fake_close(handle);
			}

			bool alive () const override;
		};
	} // namespace fake
} // namespace transport
} // namespace nano
