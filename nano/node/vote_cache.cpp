#include "nano/secure/common.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/vote_cache.hpp>

#include <memory>
#include <vector>

/*
 * vote_cache
 */

nano::vote_cache::vote_cache (rsnano::VoteCacheHandle * handle) :
	handle{ handle }
{
}

nano::vote_cache::vote_cache (vote_cache_config const & config_a, nano::stats & stats_a)
{
	auto config_dto{ config_a.to_dto () };
	handle = rsnano::rsn_vote_cache_create (&config_dto, stats_a.handle);
}

nano::vote_cache::~vote_cache ()
{
	rsnano::rsn_vote_cache_destroy (handle);
}

void nano::vote_cache::insert (
std::shared_ptr<nano::vote> const & vote,
nano::uint128_t weight,
std::function<bool (nano::block_hash const &)> const & filter)
{
	nano::amount weight_amount{ weight };
	rsnano::rsn_vote_cache_vote (handle, vote->get_handle (), weight_amount.bytes.data ());
}

bool nano::vote_cache::empty () const
{
	return rsnano::rsn_vote_cache_cache_empty (handle);
}

std::size_t nano::vote_cache::size () const
{
	return rsnano::rsn_vote_cache_cache_size (handle);
}

std::vector<std::shared_ptr<nano::vote>> nano::vote_cache::find (const nano::block_hash & hash) const
{
	return nano::into_vote_vec (rsnano::rsn_vote_cache_find (handle, hash.bytes.data ()));
}

void nano::vote_cache::clear ()
{
	return rsnano::rsn_vote_cache_clear (handle);
}

std::unique_ptr<nano::container_info_component> nano::vote_cache::collect_container_info (const std::string & name) const
{
	auto info_handle = rsnano::rsn_vote_cache_collect_container_info (handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}

/*
 * vote_cache_config
 */

nano::vote_cache_config::vote_cache_config (rsnano::VoteCacheConfigDto dto)
{
	max_size = dto.max_size;
	max_voters = dto.max_voters;
	age_cutoff = std::chrono::seconds{ dto.age_cutoff_s };
}

nano::error nano::vote_cache_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("max_size", max_size);
	toml.get ("max_voters", max_voters);

	auto age_cutoff_l = age_cutoff.count ();
	toml.get ("age_cutoff", age_cutoff_l);
	age_cutoff = std::chrono::seconds{ age_cutoff_l };

	return toml.get_error ();
}

rsnano::VoteCacheConfigDto nano::vote_cache_config::to_dto () const
{
	auto age_cutoff_s = static_cast<uint64_t> (age_cutoff.count ());
	return {
		max_size,
		max_voters,
		age_cutoff_s
	};
}
