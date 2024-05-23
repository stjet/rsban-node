#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/processing_queue.hpp>
#include <nano/node/bandwidth_limiter.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/secure/common.hpp>

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
	local_block_broadcaster (rsnano::LocalBlockBroadcasterHandle * handle);
	local_block_broadcaster (local_block_broadcaster const &) = delete;
	~local_block_broadcaster ();

	void start ();
	void stop ();

	rsnano::LocalBlockBroadcasterHandle * handle;
};
}
