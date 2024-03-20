#pragma once
namespace rsnano
{
class GenerateCacheHandle;
}

namespace nano
{
/* Holds flags for various cacheable data. For most CLI operations caching is unnecessary
 * (e.g getting the cemented block count) so it can be disabled for performance reasons. */
class generate_cache_flags
{
public:
	generate_cache_flags ();
	generate_cache_flags (rsnano::GenerateCacheHandle * handle_a);
	generate_cache_flags (generate_cache_flags const &);
	generate_cache_flags (generate_cache_flags && other_a) noexcept;
	~generate_cache_flags ();
	generate_cache_flags & operator= (generate_cache_flags const & other_a);
	generate_cache_flags & operator= (generate_cache_flags && other_a);
	bool reps () const;
	void enable_reps (bool enable);
	bool cemented_count () const;
	void enable_cemented_count (bool enable);
	void enable_unchecked_count (bool enable);
	bool account_count () const;
	void enable_account_count (bool enable);
	bool block_count () const;
	void enable_block_count (bool enable);
	void enable_all ();
	rsnano::GenerateCacheHandle * handle;
};
}
