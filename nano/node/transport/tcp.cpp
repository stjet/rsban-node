#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/messages.hpp"
#include "nano/secure/network_filter.hpp"

#include <nano/crypto_lib/random_pool_shuffle.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <memory>

/*
 * tcp_channels
 */

nano::transport::tcp_channels::tcp_channels (rsnano::TcpChannelsHandle * handle, rsnano::NetworkFilterHandle * filter_handle) :
	handle{ handle },
	publish_filter{ std::make_shared<nano::network_filter> (filter_handle) }
{
}

nano::transport::tcp_channels::~tcp_channels ()
{
	rsnano::rsn_tcp_channels_destroy (handle);
}

std::size_t nano::transport::tcp_channels::size () const
{
	return rsnano::rsn_tcp_channels_channel_count (handle);
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

uint16_t nano::transport::tcp_channels::port () const
{
	return rsnano::rsn_tcp_channels_port (handle);
}

void nano::transport::tcp_channels::purge (std::chrono::system_clock::time_point const & cutoff_a)
{
	uint64_t cutoff_ns = std::chrono::duration_cast<std::chrono::nanoseconds> (cutoff_a.time_since_epoch ()).count ();
	rsnano::rsn_tcp_channels_purge (handle, cutoff_ns);
}

