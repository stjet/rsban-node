#include <nano/lib/rsnano.hpp>
#include <nano/secure/ledger_cache.hpp>

nano::ledger_cache::ledger_cache (rsnano::LedgerCacheHandle * handle_a) :
	handle{ handle_a }
{
}

nano::ledger_cache::ledger_cache (ledger_cache && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::ledger_cache::~ledger_cache ()
{
	if (handle != nullptr)
		rsnano::rsn_ledger_cache_destroy (handle);
}

nano::ledger_cache & nano::ledger_cache::operator= (nano::ledger_cache && other_a)
{
	if (handle != nullptr)
		rsnano::rsn_ledger_cache_destroy (handle);
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}

uint64_t nano::ledger_cache::cemented_count () const
{
	return rsnano::rsn_ledger_cache_cemented_count (handle);
}
uint64_t nano::ledger_cache::block_count () const
{
	return rsnano::rsn_ledger_cache_block_count (handle);
}
uint64_t nano::ledger_cache::pruned_count () const
{
	return rsnano::rsn_ledger_cache_pruned_count (handle);
}
uint64_t nano::ledger_cache::account_count () const
{
	return rsnano::rsn_ledger_cache_account_count (handle);
}
void nano::ledger_cache::add_cemented (uint64_t count)
{
	rsnano::rsn_ledger_cache_add_cemented (handle, count);
}
void nano::ledger_cache::add_blocks (uint64_t count)
{
	rsnano::rsn_ledger_cache_add_blocks (handle, count);
}
void nano::ledger_cache::add_pruned (uint64_t count)
{
	rsnano::rsn_ledger_cache_add_pruned (handle, count);
}
void nano::ledger_cache::add_accounts (uint64_t count)
{
	rsnano::rsn_ledger_cache_add_accounts (handle, count);
}
void nano::ledger_cache::remove_blocks (uint64_t count)
{
	rsnano::rsn_ledger_cache_remove_blocks (handle, count);
}
void nano::ledger_cache::remove_accounts (uint64_t count)
{
	rsnano::rsn_ledger_cache_remove_accounts (handle, count);
}
