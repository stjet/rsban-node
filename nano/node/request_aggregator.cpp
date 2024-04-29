#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/common.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/request_aggregator.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

nano::request_aggregator::request_aggregator (nano::node_config const & config_a, nano::stats & stats_a,
nano::vote_generator & generator_a, nano::vote_generator & final_generator_a,
nano::local_vote_history & history_a, nano::ledger & ledger_a, nano::wallets & wallets_a,
nano::active_transactions & active_a)
{
	auto config_dto{ config_a.to_dto () };
	handle = rsnano::rsn_request_aggregator_create (&config_dto, stats_a.handle, generator_a.handle,
	final_generator_a.handle, history_a.handle, ledger_a.handle, wallets_a.rust_handle,
	active_a.handle, config_a.network_params.network.is_dev_network ());
	rsnano::rsn_request_aggregator_start (handle);
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

void nano::request_aggregator::stop ()
{
	rsnano::rsn_request_aggregator_stop (handle);
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

std::unique_ptr<nano::container_info_component> nano::collect_container_info (nano::request_aggregator & aggregator, std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_request_aggregator_collect_container_info (aggregator.handle, name.c_str ()));
}
