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
#include <stdexcept>
#include <vector>

/*
 * vote_cache_entry
 */

nano::vote_cache_entry::vote_cache_entry (rsnano::VoteCacheEntryHandle * handle) :
	handle{ handle }
{
}

nano::vote_cache_entry::~vote_cache_entry ()
{
	rsnano::rsn_vote_cache_entry_destroy (handle);
}

std::size_t nano::vote_cache_entry::size () const
{
	return rsnano::rsn_vote_cache_entry_size (handle);
}

nano::block_hash nano::vote_cache_entry::hash () const
{
	nano::block_hash result;
	rsnano::rsn_vote_cache_entry_hash (handle, result.bytes.data ());
	return result;
}

nano::uint128_t nano::vote_cache_entry::tally () const
{
	nano::amount result;
	rsnano::rsn_vote_cache_entry_tally (handle, result.bytes.data ());
	return result.number ();
}

nano::uint128_t nano::vote_cache_entry::final_tally () const
{
	nano::amount result;
	rsnano::rsn_vote_cache_entry_final_tally (handle, result.bytes.data ());
	return result.number ();
}

std::vector<std::shared_ptr<nano::vote>> nano::vote_cache_entry::votes () const
{
	return nano::into_vote_vec (rsnano::rsn_vote_cache_entry_votes (handle));
}

/*
 * vote_cache
 */

nano::vote_cache::vote_cache (vote_cache_config const & config_a, nano::stats & stats_a)
{
	auto config_dto{ config_a.to_dto () };
	handle = rsnano::rsn_vote_cache_create (&config_dto, stats_a.handle);
}

nano::vote_cache::~vote_cache ()
{
	rsnano::rsn_vote_cache_destroy (handle);
}

void nano::vote_cache::observe (const std::shared_ptr<nano::vote> & vote, nano::uint128_t rep_weight, nano::vote_source source, std::unordered_map<nano::block_hash, nano::vote_code> results)
{
	auto results_handle = rsnano::rsn_vote_result_map_create ();
	for (auto const & it : results)
	{
		rsnano::rsn_vote_result_map_insert (results_handle, it.first.bytes.data (), static_cast<uint8_t> (it.second));
	}
	nano::amount weight{ rep_weight };
	rsnano::rsn_vote_cache_observe (handle, vote->get_handle (), weight.bytes.data (), static_cast<uint8_t> (source), results_handle);
	rsnano::rsn_vote_result_map_destroy (results_handle);
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

bool nano::vote_cache::erase (const nano::block_hash & hash)
{
	return rsnano::rsn_vote_cache_erase (handle, hash.bytes.data ());
}

void nano::vote_cache::clear ()
{
	return rsnano::rsn_vote_cache_clear (handle);
}

std::vector<nano::vote_cache::top_entry> nano::vote_cache::top (const nano::uint128_t & min_tally)
{
	nano::amount min_tally_amount{ min_tally };
	auto vec_handle = rsnano::rsn_vote_cache_top (handle, min_tally_amount.bytes.data ());

	std::vector<top_entry> results;
	auto len = rsnano::rsn_top_entry_vec_len (vec_handle);
	for (auto i = 0; i < len; ++i)
	{
		rsnano::TopEntryDto dto;
		rsnano::rsn_top_entry_vec_get (vec_handle, i, &dto);
		results.push_back ({
		nano::block_hash::from_bytes (&dto.hash[0]),
		nano::amount::from_bytes (&dto.tally[0]).number (),
		nano::amount::from_bytes (&dto.final_tally[0]).number (),
		});
	}
	rsnano::rsn_top_entry_vec_destroy (vec_handle);
	return results;
}

std::unique_ptr<nano::container_info_component> nano::vote_cache::collect_container_info (const std::string & name) const
{
	auto info_handle = rsnano::rsn_vote_cache_collect_container_info (handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}

/*
 * vote_cache_config
 */

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
