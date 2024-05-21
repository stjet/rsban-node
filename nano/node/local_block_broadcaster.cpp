#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/local_block_broadcaster.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>

nano::local_block_broadcaster::local_block_broadcaster (rsnano::LocalBlockBroadcasterHandle * handle)
	: handle{handle}
{}

nano::local_block_broadcaster::~local_block_broadcaster ()
{
	rsnano::rsn_local_block_broadcaster_destroy (handle);
}

void nano::local_block_broadcaster::start ()
{
	rsnano::rsn_local_block_broadcaster_start (handle);
}

void nano::local_block_broadcaster::stop ()
{
	rsnano::rsn_local_block_broadcaster_stop (handle);
}

std::unique_ptr<nano::container_info_component> nano::local_block_broadcaster::collect_container_info (const std::string & name) const
{
	return std::make_unique<container_info_composite> (rsnano::rsn_local_block_broadcaster_collect_container_info (handle, name.c_str ()));
}
