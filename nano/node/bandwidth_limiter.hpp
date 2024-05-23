#pragma once

#include <nano/node/transport/traffic_type.hpp>

#include <cstddef>

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

/**
 * Class that tracks and manages bandwidth limits for IO operations
 */
class bandwidth_limiter final
{
public:
	// initialize with limit 0 = unbounded
	bandwidth_limiter (std::size_t limit, double burst_ratio);
	explicit bandwidth_limiter (rsnano::BandwidthLimiterHandle * handle_a);
	bandwidth_limiter (bandwidth_limiter && other_a);
	bandwidth_limiter (bandwidth_limiter const &) = delete;
	~bandwidth_limiter ();

public:
	rsnano::BandwidthLimiterHandle * handle;
};

class outbound_bandwidth_limiter final
{
public: // Config
	struct config
	{
		// standard
		std::size_t standard_limit;
		double standard_burst_ratio;
		// bootstrap
		std::size_t bootstrap_limit;
		double bootstrap_burst_ratio;
	};

public:
	explicit outbound_bandwidth_limiter (config);
	explicit outbound_bandwidth_limiter (rsnano::OutboundBandwidthLimiterHandle * handle);
	outbound_bandwidth_limiter (outbound_bandwidth_limiter const &) = delete;
	outbound_bandwidth_limiter (outbound_bandwidth_limiter &&) = delete;
	~outbound_bandwidth_limiter ();

	rsnano::OutboundBandwidthLimiterHandle * handle;
};
}
