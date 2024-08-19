#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

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
}
