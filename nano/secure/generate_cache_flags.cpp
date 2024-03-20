#include <nano/lib/rsnano.hpp>
#include <nano/secure/generate_cache_flags.hpp>

nano::generate_cache_flags::generate_cache_flags () :
	handle{ rsnano::rsn_generate_cache_create () }
{
}

nano::generate_cache_flags::generate_cache_flags (rsnano::GenerateCacheHandle * handle_a) :
	handle{ handle_a }
{
}

void nano::generate_cache_flags::enable_all ()
{
	rsnano::rsn_generate_cache_enable_all (handle);
}

nano::generate_cache_flags::generate_cache_flags (nano::generate_cache_flags && other_a) noexcept :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::generate_cache_flags::generate_cache_flags (const nano::generate_cache_flags & other_a) :
	handle{ rsnano::rsn_generate_cache_clone (other_a.handle) }
{
}

nano::generate_cache_flags::~generate_cache_flags ()
{
	if (handle)
		rsnano::rsn_generate_cache_destroy (handle);
}

nano::generate_cache_flags & nano::generate_cache_flags::operator= (nano::generate_cache_flags && other_a)
{
	if (handle != nullptr)
		rsnano::rsn_generate_cache_destroy (handle);
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}
nano::generate_cache_flags & nano::generate_cache_flags::operator= (const nano::generate_cache_flags & other_a)
{
	if (handle != nullptr)
		rsnano::rsn_generate_cache_destroy (handle);
	handle = rsnano::rsn_generate_cache_clone (other_a.handle);
	return *this;
}
bool nano::generate_cache_flags::reps () const
{
	return rsnano::rsn_generate_cache_reps (handle);
}
void nano::generate_cache_flags::enable_reps (bool enable)
{
	rsnano::rsn_generate_cache_set_reps (handle, enable);
}
bool nano::generate_cache_flags::cemented_count () const
{
	return rsnano::rsn_generate_cache_cemented_count (handle);
}
void nano::generate_cache_flags::enable_cemented_count (bool enable)
{
	rsnano::rsn_generate_cache_set_cemented_count (handle, enable);
}
void nano::generate_cache_flags::enable_unchecked_count (bool enable)
{
	rsnano::rsn_generate_cache_set_unchecked_count (handle, enable);
}
bool nano::generate_cache_flags::account_count () const
{
	return rsnano::rsn_generate_cache_account_count (handle);
}
void nano::generate_cache_flags::enable_account_count (bool enable)
{
	rsnano::rsn_generate_cache_set_account_count (handle, enable);
}
bool nano::generate_cache_flags::block_count () const
{
	return rsnano::rsn_generate_cache_block_count (handle);
}
void nano::generate_cache_flags::enable_block_count (bool enable)
{
	rsnano::rsn_generate_cache_set_account_count (handle, enable);
}
