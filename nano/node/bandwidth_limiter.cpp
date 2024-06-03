#include <nano/lib/rsnano.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/bandwidth_limiter.hpp>

/*
 * outbound_bandwidth_limiter
 */

nano::outbound_bandwidth_limiter::outbound_bandwidth_limiter (rsnano::OutboundBandwidthLimiterHandle * handle) :
	handle{ handle }
{
}

nano::outbound_bandwidth_limiter::~outbound_bandwidth_limiter ()
{
	rsnano::rsn_outbound_bandwidth_limiter_destroy (handle);
}

nano::bandwidth_limit_type nano::to_bandwidth_limit_type (const nano::transport::traffic_type & traffic_type)
{
	return static_cast<nano::bandwidth_limit_type> (rsnano::rsn_traffic_type_to_bandwidth_limit_type (static_cast<uint8_t> (traffic_type)));
}
