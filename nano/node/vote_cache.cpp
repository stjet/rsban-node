#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/utility.hpp"

#include <nano/node/node.hpp>
#include <nano/node/vote_cache.hpp>

#include <memory>
#include <vector>

nano::vote_cache::entry::entry (const nano::block_hash & hash) :
	hash{ hash }
{
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

nano::vote_cache::vote_cache (const config config_a) :
	handle{ rsnano::rsn_vote_cache_create (config_a.max_size) }
{
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
nano::vote_cache::entry entry_from_dto (rsnano::VoteCacheEntryDto & dto)
{
	nano::vote_cache::entry entry{ nano::block_hash::from_bytes (&dto.hash[0]) };
	nano::amount tally;
	std::copy (std::begin (dto.tally), std::end (dto.tally), std::begin (tally.bytes));
	entry.tally = tally.number ();
	entry.voters.reserve (dto.voters_count);
	for (auto i = 0; i < dto.voters_count; ++i)
	{
		nano::account account;
		uint64_t timestamp;
		rsnano::rsn_vote_cache_entry_get_voter (&dto, i, account.bytes.data (), &timestamp);
		entry.voters.emplace_back (account, timestamp);
	}
	rsnano::rsn_vote_cache_entry_destroy (&dto);
	return entry;
}
}

std::optional<nano::vote_cache::entry> nano::vote_cache::find (const nano::block_hash & hash) const
{
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_find (handle, hash.bytes.data (), &result))
	{
		return entry_from_dto (result);
	}
	return {};
}

bool nano::vote_cache::erase (const nano::block_hash & hash)
{
	return rsnano::rsn_vote_cache_erase (handle, hash.bytes.data ());
}

std::optional<nano::vote_cache::entry> nano::vote_cache::pop (nano::uint128_t const & min_tally)
{
	nano::amount min_tally_amount{ min_tally };
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_pop (handle, min_tally_amount.bytes.data (), &result))
	{
		return entry_from_dto (result);
	}
	return {};
}

std::optional<nano::vote_cache::entry> nano::vote_cache::peek (nano::uint128_t const & min_tally) const
{
	nano::amount min_tally_amount{ min_tally };
	rsnano::VoteCacheEntryDto result{};
	if (rsnano::rsn_vote_cache_peek (handle, min_tally_amount.bytes.data (), &result))
	{
		return entry_from_dto (result);
	}
	return {};
}

void nano::vote_cache::trigger (const nano::block_hash & hash)
{
	rsnano::rsn_vote_cache_trigger (handle, hash.bytes.data ());
}

std::unique_ptr<nano::container_info_component> nano::vote_cache::collect_container_info (const std::string & name)
{
	auto info_handle = rsnano::rsn_vote_cache_collect_container_info (handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}