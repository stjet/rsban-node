#pragma once

#include "nano/lib/interval.hpp"
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
}

namespace nano
{
class vote_cache_config final
{
public:
	nano::error deserialize (nano::tomlconfig & toml);
	rsnano::VoteCacheConfigDto to_dto () const;

public:
	std::size_t max_size{ 1024 * 128 };
	std::size_t max_voters{ 128 };
	std::chrono::seconds age_cutoff{ 5 * 60 };
};

class vote_cache final
{
public:
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
	explicit vote_cache (vote_cache_config const &, nano::stats &);
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

	std::size_t size () const;
	bool empty () const;

	rsnano::VoteCacheHandle * handle;

public:
	struct top_entry
	{
		nano::block_hash hash;
		nano::uint128_t tally;
		nano::uint128_t final_tally;
	};

	/**
	 * Returns blocks with highest observed tally
	 * The blocks are sorted in descending order by final tally, then by tally
	 * @param min_tally minimum tally threshold, entries below with their voting weight below this will be ignored
	 */
	std::vector<top_entry> top (nano::uint128_t const & min_tally);

public: // Container info
	std::unique_ptr<nano::container_info_component> collect_container_info (std::string const & name) const;
};
}
