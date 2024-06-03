#pragma once

#include <nano/node/transport/traffic_type.hpp>

namespace rsnano
{
class OutboundBandwidthLimiterHandle;
class BandwidthLimiterHandle;
}

namespace nano
{
/**
 * Enumeration for different bandwidth limits for different traffic types
 */
enum class bandwidth_limit_type
{
	/** For all message */
	standard,
	/** For bootstrap (asc_pull_ack, asc_pull_req) traffic */
	bootstrap
};

nano::bandwidth_limit_type to_bandwidth_limit_type (nano::transport::traffic_type const &);

class outbound_bandwidth_limiter final
{
public:
	explicit outbound_bandwidth_limiter (rsnano::OutboundBandwidthLimiterHandle * handle);
	outbound_bandwidth_limiter (outbound_bandwidth_limiter const &) = delete;
	outbound_bandwidth_limiter (outbound_bandwidth_limiter &&) = delete;
	~outbound_bandwidth_limiter ();

	rsnano::OutboundBandwidthLimiterHandle * handle;
};
}
