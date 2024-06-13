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

class vote_cache final
{
public:
	explicit vote_cache (rsnano::VoteCacheHandle * handle);
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
	 * Tries to find an entry associated with block hash
	 */
	std::vector<std::shared_ptr<nano::vote>> find (nano::block_hash const & hash) const;

	void clear ();

	std::size_t size () const;
	bool empty () const;

public: // Container info
	rsnano::VoteCacheHandle * handle;
};
}
