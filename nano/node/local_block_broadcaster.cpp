#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/local_block_broadcaster.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>

nano::local_block_broadcaster::local_block_broadcaster (nano::node & node_a, nano::block_processor & block_processor_a, nano::network & network_a, nano::stats & stats_a, bool enabled_a) :
	handle{ rsnano::rsn_local_block_broadcaster_create (block_processor_a.handle, stats_a.handle,
	network_a.tcp_channels->handle, node_a.representative_register.handle,
	node_a.ledger.handle, node_a.confirming_set.handle, enabled_a) }
{
	rsnano::rsn_local_block_broadcaster_initialize (handle);
}

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
