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
 * entry
 */

nano::vote_cache::entry::entry (const nano::block_hash & hash) :
	hash_m{ hash }
{
}

nano::vote_cache::entry::entry (rsnano::VoteCacheEntryDto & dto) :
	hash_m{ nano::block_hash::from_bytes (&dto.hash[0]) }
{
	nano::amount tally;
	nano::amount final_tally;
	std::copy (std::begin (dto.tally), std::end (dto.tally), std::begin (tally.bytes));
	std::copy (std::begin (dto.final_tally), std::end (dto.final_tally), std::begin (final_tally.bytes));
	tally_m = tally.number ();
	final_tally_m = final_tally.number ();
	voters_m.reserve (dto.voters_count);
	for (auto i = 0; i < dto.voters_count; ++i)
	{
		nano::vote_cache::entry::voter_entry e;
		rsnano::rsn_vote_cache_entry_get_voter (&dto, i, e.representative.bytes.data (), &e.timestamp);
		voters_m.push_back (e);
	}
	rsnano::rsn_vote_cache_entry_destroy (&dto);
}

std::size_t nano::vote_cache::entry::size () const
{
	return voters_m.size ();
}

nano::block_hash nano::vote_cache::entry::hash () const
{
	return hash_m;
}

nano::uint128_t nano::vote_cache::entry::tally () const
{
	return tally_m;
}

nano::uint128_t nano::vote_cache::entry::final_tally () const
{
	return final_tally_m;
}

std::vector<nano::vote_cache::entry::voter_entry> nano::vote_cache::entry::voters () const
{
	return voters_m;
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

void nano::vote_cache::vote (const nano::block_hash & hash, const std::shared_ptr<nano::vote> vote, nano::uint128_t rep_weight)
{
	nano::amount rep_weight_amount{ rep_weight };
	rsnano::rsn_vote_cache_vote (handle, hash.bytes.data (), vote->get_handle (), rep_weight_amount.bytes.data ());
}

bool nano::vote_cache::empty () const
{
	return rsnano::rsn_vote_cache_cache_empty (handle);
}

std::size_t nano::vote_cache::size () const
{
	return rsnano::rsn_vote_cache_cache_size (handle);
}

std::optional<nano::vote_cache::entry> nano::vote_cache::find (const nano::block_hash & hash) const
{
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_find (handle, hash.bytes.data (), &result))
	{
		return nano::vote_cache::entry{ result };
	}
	return {};
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
