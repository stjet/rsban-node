#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <memory>
#include <vector>

namespace rsnano
{
class VoteCacheEntryHandle;
}

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
	vote_cache_config () = default;
	explicit vote_cache_config (rsnano::VoteCacheConfigDto dto);
	nano::error deserialize (nano::tomlconfig & toml);
	rsnano::VoteCacheConfigDto to_dto () const;

public:
	std::size_t max_size{ 1024 * 64 };
	std::size_t max_voters{ 64 };
	std::chrono::seconds age_cutoff{ 15 * 60 };
};

/**
 * Stores votes associated with a single block hash
 */
class vote_cache_entry final
{
public:
	explicit vote_cache_entry (rsnano::VoteCacheEntryHandle * handle);
	vote_cache_entry (vote_cache_entry const &) = delete;
	vote_cache_entry (vote_cache_entry &&) = delete;
	~vote_cache_entry ();

	std::size_t size () const;

	nano::block_hash hash () const;
	nano::uint128_t tally () const;
	nano::uint128_t final_tally () const;
	std::vector<std::shared_ptr<nano::vote>> votes () const;
	rsnano::VoteCacheEntryHandle * handle;
};

class vote_cache final
{
public:
	using entry = vote_cache_entry;

public:
	explicit vote_cache (vote_cache_config const &, nano::stats &);
	vote_cache (vote_cache const &) = delete;
	vote_cache (vote_cache &&) = delete;
	~vote_cache ();

	/**
	 * Adds a new vote to cache
	 */
	void insert (
	std::shared_ptr<nano::vote> const & vote,
	nano::uint128_t weight,
	std::function<bool (nano::block_hash const &)> const & filter = [] (nano::block_hash const &) { return true; });

	/**
	 * Should be called for every processed vote, filters which votes should be added to cache
	 */
	void observe (std::shared_ptr<nano::vote> const & vote, nano::uint128_t rep_weight, nano::vote_source source, std::unordered_map<nano::block_hash, nano::vote_code>);

	/**
	 * Tries to find an entry associated with block hash
	 */
	std::vector<std::shared_ptr<nano::vote>> find (nano::block_hash const & hash) const;

	/**
	 * Removes an entry associated with block hash, does nothing if entry does not exist
	 * @return true if hash existed and was erased, false otherwise
	 */
	bool erase (nano::block_hash const & hash);
	void clear ();

	std::size_t size () const;
	bool empty () const;

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

	rsnano::VoteCacheHandle * handle;
};
}
