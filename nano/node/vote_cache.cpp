#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/node/node.hpp>
#include <nano/node/vote_cache.hpp>
#include <vector>

nano::vote_cache::entry::entry (const nano::block_hash & hash) :
	hash{ hash }
{
}

bool nano::vote_cache::entry::vote (const nano::account & representative, const uint64_t & timestamp, const nano::uint128_t & rep_weight)
{
	auto existing = std::find_if (voters.begin (), voters.end (), [&representative] (auto const & item) { return item.first == representative; });
	if (existing != voters.end ())
	{
		// We already have a vote from this rep
		// Update timestamp if newer but tally remains unchanged as we already counted this rep weight
		// It is not essential to keep tally up to date if rep voting weight changes, elections do tally calculations independently, so in the worst case scenario only our queue ordering will be a bit off
		if (timestamp > existing->second)
		{
			existing->second = timestamp;
		}
		return false;
	}
	else
	{
		// Vote from an unseen representative, add to list and update tally
		if (voters.size () < max_voters)
		{
			voters.emplace_back (representative, timestamp);
			tally += rep_weight;
			return true;
		}
		else
		{
			return false;
		}
	}
}

std::size_t nano::vote_cache::entry::size () const
{
	return voters.size ();
}

namespace
{
void execute_rep_weight_query (void * handle_a, uint8_t const * account_a, uint8_t * amount_a)
{
	auto fp = static_cast<std::function<nano::uint128_t (nano::account const &)> *> (handle_a);
	nano::account acc{};
	std::copy (account_a, account_a + 32, std::begin (acc.bytes));
	nano::amount weight{ (*fp) (acc) };
	std::copy (std::begin (weight.bytes), std::end (weight.bytes), amount_a);
}

void delete_rep_weight_query (void * handle_a)
{
	auto fp = static_cast<std::function<nano::uint128_t (nano::account const &)> *> (handle_a);
	delete fp;
}
}

nano::vote_cache::vote_cache (const config config_a, std::function<nano::uint128_t (nano::account const &)> rep_weight_query_a) :
	max_size{ config_a.max_size },
	rep_weight_query{ rep_weight_query_a },
	handle{ rsnano::rsn_vote_cache_create (
	config_a.max_size,
	new std::function<nano::uint128_t (nano::account const &)> (rep_weight_query_a),
	execute_rep_weight_query,
	delete_rep_weight_query) }
{
}

nano::vote_cache::~vote_cache ()
{
	rsnano::rsn_vote_cache_destroy (handle);
}

void nano::vote_cache::vote (const nano::block_hash & hash, const std::shared_ptr<nano::vote> vote)
{
	rsnano::rsn_vote_cache_vote (handle, hash.bytes.data (), vote->get_handle ());
}

bool nano::vote_cache::cache_empty () const
{
	return rsnano::rsn_vote_cache_cache_empty (handle);
}

bool nano::vote_cache::queue_empty () const
{
	return rsnano::rsn_vote_cache_queue_empty (handle);
}

std::size_t nano::vote_cache::cache_size () const
{
	return rsnano::rsn_vote_cache_cache_size (handle);
}

std::size_t nano::vote_cache::queue_size () const
{
	return rsnano::rsn_vote_cache_queue_size (handle);
}

namespace
{
	nano::vote_cache::entry entry_from_dto(rsnano::VoteCacheEntryDto& dto)
	{
		nano::vote_cache::entry entry{ nano::block_hash::from_bytes (&dto.hash[0]) };
		nano::amount tally;
		std::copy(std::begin(dto.tally), std::end(dto.tally), std::begin(tally.bytes));
		entry.tally = tally.number();
		std::vector<std::pair<nano::account, uint64_t>> voters;
		voters.reserve(dto.voters_count);
		for (auto i = 0; i < dto.voters_count; ++i) {
			nano::account account;
			uint64_t timestamp;
			rsnano::rsn_vote_cache_entry_get_voter(&dto, i, account.bytes.data(), &timestamp);
			voters.emplace_back(account, timestamp);
		}
		rsnano::rsn_vote_cache_entry_destroy(&dto);
		return entry;
	}
}

std::optional<nano::vote_cache::entry> nano::vote_cache::find (const nano::block_hash & hash) const
{
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_find (handle, hash.bytes.data (), &result))
	{
		return entry_from_dto(result);
	}
	return { };
}

bool nano::vote_cache::erase (const nano::block_hash & hash)
{
	return rsnano::rsn_vote_cache_erase (handle, hash.bytes.data ());
}

std::optional<nano::vote_cache::entry> nano::vote_cache::pop (nano::uint128_t const & min_tally)
{
	nano::amount min_tally_amount {min_tally};
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_pop(handle, min_tally_amount.bytes.data(), &result)){
		return entry_from_dto(result);
	}
	return {};
}

std::optional<nano::vote_cache::entry> nano::vote_cache::peek (nano::uint128_t const & min_tally) const
{
	nano::amount min_tally_amount {min_tally};
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_peek(handle, min_tally_amount.bytes.data(), &result)){
		return entry_from_dto(result);
	}
	return {};
}

void nano::vote_cache::trigger (const nano::block_hash & hash)
{
	rsnano::rsn_vote_cache_trigger(handle, hash.bytes.data());
}

std::unique_ptr<nano::container_info_component> nano::vote_cache::collect_container_info (const std::string & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "cache", cache_size (), sizeof (ordered_cache::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "queue", queue_size (), sizeof (ordered_queue::value_type) }));
	return composite;
}