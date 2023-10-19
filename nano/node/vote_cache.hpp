#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <memory>
#include <optional>
#include <vector>

namespace nano
{
class node;
class active_transactions;
class vote;

class vote_cache final
{
public:
	class config final
	{
	public:
		std::size_t max_size;
	};

	/**
	 * Stores votes associated with a single block hash
	 */
	class entry final
	{
	public:
		struct voter_entry
		{
			nano::account representative;
			uint64_t timestamp;
		};

	public:
		explicit entry (nano::block_hash const & hash);
		explicit entry (rsnano::VoteCacheEntryDto & dto);

		std::size_t size () const;

		nano::block_hash hash () const;
		nano::uint128_t tally () const;
		nano::uint128_t final_tally () const;
		std::vector<voter_entry> voters () const;

		nano::block_hash const hash_m;
		std::vector<voter_entry> voters_m;

		nano::uint128_t tally_m{ 0 };
		nano::uint128_t final_tally_m{ 0 };
	};

public:
	explicit vote_cache (const config);
	vote_cache (vote_cache const &) = delete;
	vote_cache (vote_cache &&) = delete;
	~vote_cache ();

	/**
	 * Adds a new vote to cache
	 */
	void vote (nano::block_hash const & hash, std::shared_ptr<nano::vote> vote, nano::uint128_t rep_weight);
	/**
	 * Tries to find an entry associated with block hash
	 */
	std::optional<entry> find (nano::block_hash const & hash) const;
	/**
	 * Removes an entry associated with block hash, does nothing if entry does not exist
	 * @return true if hash existed and was erased, false otherwise
	 */
	bool erase (nano::block_hash const & hash);
	/**
	 * Returns an entry with the highest tally.
	 * @param min_tally minimum tally threshold, entries below with their voting weight below this will be ignored
	 */
	std::optional<entry> peek (nano::uint128_t const & min_tally = 0) const;
	/**
	 * Returns an entry with the highest tally and removes it from container.
	 * @param min_tally minimum tally threshold, entries below with their voting weight below this will be ignored
	 */
	std::optional<entry> pop (nano::uint128_t const & min_tally = 0);
	/**
	 * Reinserts a block into the queue.
	 * It is possible that we dequeue a hash that doesn't have a received block yet (for eg. if publish message was lost).
	 * We need a way to reinsert that hash into the queue when we finally receive the block
	 */
	void trigger (const nano::block_hash & hash);

	std::size_t cache_size () const;
	std::size_t queue_size () const;
	bool cache_empty () const;
	bool queue_empty () const;

	rsnano::VoteCacheHandle * handle;

public:
	struct top_entry
	{
		nano::block_hash hash;
		nano::uint128_t tally;
		nano::uint128_t final_tally;
	};

	/**
	 * Returns blocks with highest observed tally, greater than `min_tally`
	 */
	std::vector<top_entry> top (nano::uint128_t const & min_tally) const;

public: // Container info
	std::unique_ptr<nano::container_info_component> collect_container_info (std::string const & name);
};
}
