#pragma once

#include <cstdint>
namespace rsnano
{
class LedgerCacheHandle;
}

namespace nano
{
/* Holds an in-memory cache of various counts */
class ledger_cache
{
public:
	ledger_cache (rsnano::LedgerCacheHandle * handle_a);
	ledger_cache (ledger_cache &&);
	~ledger_cache ();
	ledger_cache (ledger_cache const &) = delete;
	ledger_cache & operator= (ledger_cache && other_a);
	uint64_t cemented_count () const;
	void add_cemented (uint64_t count);
	uint64_t block_count () const;
	void add_blocks (uint64_t count);
	void remove_blocks (uint64_t count);
	uint64_t pruned_count () const;
	void add_pruned (uint64_t count);
	uint64_t account_count () const;
	void add_accounts (uint64_t count);
	void remove_accounts (uint64_t count);
	rsnano::LedgerCacheHandle * handle;
};
}
