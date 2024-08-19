#include <nano/node/vote_cache.hpp>

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
