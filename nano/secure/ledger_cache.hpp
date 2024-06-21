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
	uint64_t block_count () const;
	uint64_t pruned_count () const;
	uint64_t account_count () const;
	rsnano::LedgerCacheHandle * handle;
};
}
