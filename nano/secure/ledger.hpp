#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/timer.hpp>
#include <nano/secure/account_info.hpp>
#include <nano/secure/generate_cache_flags.hpp>
#include <nano/secure/ledger_cache.hpp>
#include <nano/secure/pending_info.hpp>
#include <nano/store/write_queue.hpp>

#include <deque>
#include <map>

namespace rsnano
{
class LedgerHandle;
}

namespace nano::store
{
class component;
class transaction;
class write_transaction;
}

namespace nano
{
class block;
enum class block_status;
enum class epoch : uint8_t;
class ledger_constants;
class pending_info;
class pending_key;
class stats;

// map of vote weight per block, ordered greater first
using tally_t = std::map<nano::uint128_t, std::shared_ptr<nano::block>, std::greater<nano::uint128_t>>;

class ledger_set_any
{
public:
	ledger_set_any (rsnano::LedgerSetAnyHandle * handle);
	~ledger_set_any ();

	std::optional<nano::account_info> account_get (store::transaction const & transaction, nano::account const & account) const;
	bool block_exists_or_pruned (store::transaction const & transaction, nano::block_hash const & hash) const;
	bool block_exists (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::shared_ptr<nano::block> block_get (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::optional<nano::amount> block_balance (store::transaction const & transaction, nano::block_hash const & hash) const;
	nano::block_hash account_head (store::transaction const & transaction, nano::account const & account) const;
	std::optional<nano::account> block_account (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::optional<nano::amount> block_amount (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::optional<nano::amount> account_balance (store::transaction const & transaction, nano::account const & account) const;
	std::optional<nano::pending_info> pending_get (store::transaction const & transaction, nano::pending_key const & key) const;
	std::optional<nano::block_hash> block_successor (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::optional<nano::block_hash> block_successor (store::transaction const & transaction, nano::qualified_root const & root) const;
	nano::receivable_iterator receivable_upper_bound (store::transaction const & transaction, nano::account const & account, nano::block_hash const & hash) const;
	nano::receivable_iterator receivable_upper_bound (store::transaction const & transaction, nano::account const & account) const;

	rsnano::LedgerSetAnyHandle * handle;
};

class ledger_set_confirmed
{
public:
	ledger_set_confirmed (rsnano::LedgerSetConfirmedHandle * handle);
	~ledger_set_confirmed ();

	bool block_exists_or_pruned (store::transaction const & transaction, nano::block_hash const & hash) const;
	bool block_exists (store::transaction const & transaction, nano::block_hash const & hash) const;
	std::optional<nano::amount> account_balance (store::transaction const & transaction, nano::account const & account) const;

	rsnano::LedgerSetConfirmedHandle * handle;
};

class ledger final
{
public:
	ledger (nano::store::component &, nano::stats &, nano::ledger_constants & constants, nano::generate_cache_flags const & = nano::generate_cache_flags (), nano::uint128_t min_rep_weight_a = 0);
	ledger (rsnano::LedgerHandle * handle, nano::store::component &, nano::ledger_constants & constants);
	ledger (nano::ledger const &) = delete;
	ledger (nano::ledger &&) = delete;
	~ledger ();

	ledger_set_any any () const;
	ledger_set_confirmed confirmed () const;

	[[nodiscard ("write_guard blocks other waiters")]] nano::store::write_guard wait (nano::store::writer writer);
	/** Returns true if this writer is anywhere in the queue. Currently only used in tests */
	bool queue_contains (nano::store::writer writer);
	nano::uint128_t account_receivable (store::transaction const &, nano::account const &, bool = false);
	/**
	 * Returns the cached vote weight for the given representative.
	 * If the weight is below the cache limit it returns 0.
	 * During bootstrap it returns the preconfigured bootstrap weights.
	 */
	nano::uint128_t weight (nano::account const &) const;
	/* Returns the exact vote weight for the given representative by doing a database lookup */
	nano::uint128_t weight_exact (store::transaction const &, nano::account const &) const;
	nano::root latest_root (store::transaction const &, nano::account const &);
	nano::block_hash representative (store::transaction const &, nano::block_hash const &);
	std::string block_text (char const *);
	std::string block_text (nano::block_hash const &);
	std::pair<nano::block_hash, nano::block_hash> hash_root_random (store::transaction const &) const;
	std::deque<std::shared_ptr<nano::block>> confirm (nano::store::write_transaction const & transaction, nano::block_hash const & hash);
	nano::block_status process (store::write_transaction const & transaction, std::shared_ptr<nano::block> block);
	bool rollback (store::write_transaction const &, nano::block_hash const &, std::vector<std::shared_ptr<nano::block>> &);
	bool rollback (store::write_transaction const &, nano::block_hash const &);
	void update_account (store::write_transaction const &, nano::account const &, nano::account_info const &, nano::account_info const &);
	uint64_t pruning_action (store::write_transaction &, nano::block_hash const &, uint64_t const);
	bool dependents_confirmed (store::transaction const &, nano::block const &) const;
	bool is_epoch_link (nano::link const &) const;
	std::array<nano::block_hash, 2> dependent_blocks (store::transaction const &, nano::block const &) const;
	std::shared_ptr<nano::block> find_receive_block_by_send_hash (store::transaction const & transaction, nano::account const & destination, nano::block_hash const & send_block_hash);
	nano::account epoch_signer (nano::link const &) const;
	nano::link epoch_link (nano::epoch) const;
	bool bootstrap_weight_reached () const;
	rsnano::LedgerHandle * get_handle () const;
	size_t get_bootstrap_weights_size () const;
	void enable_pruning ();
	bool pruning_enabled () const;
	std::unordered_map<nano::account, nano::uint128_t> get_bootstrap_weights () const;
	void set_bootstrap_weights (std::unordered_map<nano::account, nano::uint128_t> const & weights_a);
	void set_bootstrap_weight_max_blocks (uint64_t max_a);
	uint64_t get_bootstrap_weight_max_blocks () const;
	static nano::epoch version (nano::block const & block);
	nano::epoch version (store::transaction const & transaction, nano::block_hash const & hash) const;
	// Returns whether there are any receivable entries for 'account'
	bool receivable_any (store::transaction const & tx, nano::account const & account) const;
	uint64_t cemented_count () const;
	uint64_t block_count () const;
	uint64_t account_count () const;
	uint64_t pruned_count () const;
	static nano::uint128_t const unit;
	nano::store::component & store;
	rsnano::LedgerHandle * handle;
	nano::ledger_cache cache;
	nano::ledger_constants & constants;
};
}
