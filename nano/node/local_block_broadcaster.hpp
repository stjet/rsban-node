#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/processing_queue.hpp>
#include <nano/node/bandwidth_limiter.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/secure/common.hpp>

#include <memory>

namespace nano
{
class node;
class network;
}

namespace nano
{
/**
 * Broadcasts blocks to the network
 * Tracks local blocks for more aggressive propagation
 */
class local_block_broadcaster
{
	enum class broadcast_strategy
	{
		normal,
		aggressive,
	};

public:
	local_block_broadcaster (nano::node &, nano::block_processor &, nano::network &, nano::stats &, bool enabled = false);
	local_block_broadcaster (local_block_broadcaster const &) = delete;
	~local_block_broadcaster ();

	void start ();
	void stop ();

	std::unique_ptr<container_info_component> collect_container_info (std::string const & name) const;

	rsnano::LocalBlockBroadcasterHandle * handle;
};
}
