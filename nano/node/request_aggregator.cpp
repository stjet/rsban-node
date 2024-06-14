#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/common.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/request_aggregator.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

nano::request_aggregator::request_aggregator (rsnano::RequestAggregatorHandle * handle) :
	handle{ handle }
{
}

nano::request_aggregator::~request_aggregator ()
{
	rsnano::rsn_request_aggregator_destroy (handle);
}

void nano::request_aggregator::add (std::shared_ptr<nano::transport::channel> const & channel_a, std::vector<std::pair<nano::block_hash, nano::root>> const & hashes_roots_a)
{
	auto vec_handle = rsnano::rsn_hashes_roots_vec_create ();
	for (auto const & [hash, root] : hashes_roots_a)
	{
		rsnano::rsn_hashes_roots_vec_push (vec_handle, hash.bytes.data (), root.bytes.data ());
	}
	rsnano::rsn_request_aggregator_add (handle, channel_a->handle, vec_handle);
	rsnano::rsn_hashes_roots_vec_destroy (vec_handle);
}

std::size_t nano::request_aggregator::size ()
{
	return rsnano::rsn_request_aggregator_len (handle);
}

bool nano::request_aggregator::empty ()
{
	return size () == 0;
}

std::chrono::milliseconds nano::request_aggregator::get_max_delay () const
{
	return std::chrono::milliseconds{ rsnano::rsn_request_aggregator_max_delay_ms (handle) };
}
