#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/node/gap_cache.hpp>
#include <nano/node/node.hpp>
#include <nano/secure/store.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <cstdint>
#include <vector>

namespace
{
class gap_cache_bootstrap_starter
{
public:
	gap_cache_bootstrap_starter (nano::node & node_a) :
		node{ node_a }
	{
	}

	void bootstrap_start (nano::block_hash const & hash_a)
	{
		auto node_l (node.shared ());
		node.workers->add_timed_task (std::chrono::steady_clock::now () + node.network_params.bootstrap.gap_cache_bootstrap_start_interval, [node_l, hash_a] () {
			if (!node_l->ledger.block_or_pruned_exists (hash_a))
			{
				if (!node_l->bootstrap_initiator.in_progress ())
				{
					node_l->logger->try_log (boost::str (boost::format ("Missing block %1% which has enough votes to warrant lazy bootstrapping it") % hash_a.to_string ()));
				}
				if (!node_l->flags.disable_lazy_bootstrap ())
				{
					node_l->bootstrap_initiator.bootstrap_lazy (hash_a);
				}
				else if (!node_l->flags.disable_legacy_bootstrap ())
				{
					node_l->bootstrap_initiator.bootstrap ();
				}
			}
		});
	}

private:
	nano::node & node;
};

void start_bootstrap_callback_wrapper (void * context, const uint8_t * bytes)
{
	auto fn = static_cast<std::function<void (nano::block_hash const &)> *> (context);
	nano::block_hash hash;
	hash = nano::block_hash::from_bytes (bytes);
	(*fn) (hash);
}

void drop_start_bootstrap_callback (void * context_a)
{
	auto fn = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	delete fn;
}
}

nano::gap_cache::gap_cache (nano::node & node_a) :
	node (node_a)
{
	gap_cache_bootstrap_starter bootstrap_starter{ node_a };
	start_bootstrap_callback = [bootstrap_starter] (nano::block_hash const & hash_a) mutable {
		bootstrap_starter.bootstrap_start (hash_a);
	};

	auto context = new std::function<void (nano::block_hash const &)> (start_bootstrap_callback);

	handle = rsnano::rsn_gap_cache_create (
	node.config->to_dto (),
	node.online_reps.get_handle (),
	node.ledger.get_handle (),
	node.flags.handle,
	start_bootstrap_callback_wrapper,
	context,
	drop_start_bootstrap_callback);
}

nano::gap_cache::~gap_cache ()
{
	rsnano::rsn_gap_cache_destroy (handle);
}

void nano::gap_cache::add (nano::block_hash const & hash_a, std::chrono::steady_clock::time_point time_point_a)
{
	auto timepoint_ns = std::chrono::duration_cast<std::chrono::milliseconds> (time_point_a.time_since_epoch ()).count ();
	rsnano::rsn_gap_cache_add (handle, hash_a.bytes.data (), timepoint_ns);
}

void nano::gap_cache::erase (nano::block_hash const & hash_a)
{
	rsnano::rsn_gap_cache_erase (handle, hash_a.bytes.data ());
}

void nano::gap_cache::vote (std::shared_ptr<nano::vote> const & vote_a)
{
	rsnano::rsn_gap_cache_vote (handle, vote_a->get_handle ());
}

bool nano::gap_cache::bootstrap_check (std::vector<nano::account> const & voters_a, nano::block_hash const & hash_a)
{
	std::vector<uint8_t> bytes (voters_a.size () * 32);
	auto current = bytes.data ();
	for (const auto & voter : voters_a)
	{
		std::copy (std::begin (voter.bytes), std::end (voter.bytes), current);
		current += 32;
	}

	return rsnano::rsn_gap_cache_bootstrap_check (handle, bytes.size (), bytes.data (), hash_a.bytes.data ());
}

void nano::gap_cache::bootstrap_start (nano::block_hash const & hash_a)
{
	start_bootstrap_callback (hash_a);
}

nano::uint128_t nano::gap_cache::bootstrap_threshold ()
{
	nano::amount size;
	rsnano::rsn_gap_cache_bootstrap_threshold (handle, size.bytes.data ());
	return size.number ();
}

std::size_t nano::gap_cache::size ()
{
	return rsnano::rsn_gap_cache_size (handle);
}

bool nano::gap_cache::block_exists (nano::block_hash const & hash_a)
{
	return rsnano::rsn_gap_cache_block_exists (handle, hash_a.bytes.data ());
}

std::chrono::steady_clock::time_point nano::gap_cache::earliest ()
{
	auto value = rsnano::rsn_gap_cache_earliest (handle);
	return std::chrono::steady_clock::time_point (std::chrono::steady_clock::duration (value));
}

std::chrono::steady_clock::time_point nano::gap_cache::block_arrival (nano::block_hash const & hash_a)
{
	auto value = rsnano::rsn_gap_cache_block_arrival (handle, hash_a.bytes.data ());
	return std::chrono::steady_clock::time_point (std::chrono::steady_clock::duration (value));
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (gap_cache & gap_cache, std::string const & name)
{
	return std::make_unique<nano::container_info_composite> (
	rsnano::rsn_gap_cache_collect_container_info (gap_cache.handle, name.c_str ()));
}
