#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/rep_weights.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/account.hpp>
#include <nano/store/block.hpp>
#include <nano/store/component.hpp>
#include <nano/store/confirmation_height.hpp>
#include <nano/store/final.hpp>
#include <nano/store/frontier.hpp>
#include <nano/store/online_weight.hpp>
#include <nano/store/peer.hpp>
#include <nano/store/pending.hpp>
#include <nano/store/pruned.hpp>
#include <nano/store/version.hpp>

#include <boost/multiprecision/cpp_int.hpp>

#include <optional>

nano::ledger::ledger (nano::store::component & store_a, nano::stats & stat_a, nano::ledger_constants & constants, nano::generate_cache const & generate_cache_a) :
	constants{ constants },
	store{ store_a },
	stats{ stat_a }
{
	auto constants_dto{ constants.to_dto () };
	handle = rsnano::rsn_ledger_create (store_a.get_handle (), &constants_dto, stat_a.handle, generate_cache_a.handle);
	cache = nano::ledger_cache (rsnano::rsn_ledger_get_cache_handle (handle));
}

nano::ledger::~ledger ()
{
	rsnano::rsn_ledger_destroy (handle);
}

rsnano::LedgerHandle * nano::ledger::get_handle () const
{
	return handle;
}

// Balance for account containing hash
std::optional<nano::uint128_t> nano::ledger::balance (store::transaction const & transaction, nano::block_hash const & hash) const
{
	nano::amount result;
	bool found = rsnano::rsn_ledger_balance (handle, transaction.get_rust_handle (), hash.bytes.data (), result.bytes.data ());
	if (found)
	{
		return result.number ();
	}
	else
	{
		return std::nullopt;
	}
}

std::shared_ptr<nano::block> nano::ledger::block (store::transaction const & transaction, nano::block_hash const & hash) const
{
	return store.block ().get (transaction, hash);
}

bool nano::ledger::block_exists (store::transaction const & transaction, nano::block_hash const & hash) const
{
	return store.block ().exists (transaction, hash);
}

// Balance for an account by account number
nano::uint128_t nano::ledger::account_balance (store::transaction const & transaction_a, nano::account const & account_a, bool only_confirmed_a)
{
	nano::amount result;
	rsnano::rsn_ledger_account_balance (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), only_confirmed_a, result.bytes.data ());
	return result.number ();
}

nano::uint128_t nano::ledger::account_receivable (store::transaction const & transaction_a, nano::account const & account_a, bool only_confirmed_a)
{
	nano::amount result;
	rsnano::rsn_ledger_account_receivable (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), only_confirmed_a, result.bytes.data ());
	return result.number ();
}

std::optional<nano::pending_info> nano::ledger::pending_info (store::transaction const & transaction, nano::pending_key const & key) const
{
	nano::pending_info result;
	if (!store.pending ().get (transaction, key, result))
	{
		return result;
	}
	return std::nullopt;
}

nano::block_status nano::ledger::process (store::write_transaction const & transaction_a, std::shared_ptr<nano::block> block_a)
{
	rsnano::ProcessReturnDto result_dto;
	rsnano::rsn_ledger_process (handle, transaction_a.get_rust_handle (), block_a->get_handle (), &result_dto);
	return static_cast<nano::block_status> (result_dto.code);
}

nano::block_hash nano::ledger::representative (store::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	nano::block_hash result;
	rsnano::rsn_ledger_representative (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result;
}

bool nano::ledger::block_or_pruned_exists (nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_or_pruned_exists (handle, hash_a.bytes.data ());
}

bool nano::ledger::block_or_pruned_exists (store::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_or_pruned_exists_txn (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

std::string nano::ledger::block_text (char const * hash_a)
{
	return block_text (nano::block_hash (hash_a));
}

std::string nano::ledger::block_text (nano::block_hash const & hash_a)
{
	rsnano::StringDto dto;
	rsnano::rsn_ledger_block_text (handle, hash_a.bytes.data (), &dto);
	return rsnano::convert_dto_to_string (dto);
}

nano::account nano::ledger::block_destination (nano::store::transaction const & transaction_a, nano::block const & block_a)
{
	nano::account destination_l;
	rsnano::rsn_ledger_block_destination (handle, transaction_a.get_rust_handle (), block_a.get_handle (), destination_l.bytes.data ());
	return destination_l;
}

std::pair<nano::block_hash, nano::block_hash> nano::ledger::hash_root_random (store::transaction const & transaction_a) const
{
	nano::block_hash hash;
	nano::block_hash root;
	rsnano::rsn_ledger_hash_root_random (handle, transaction_a.get_rust_handle (), hash.bytes.data (), root.bytes.data ());
	return std::make_pair (hash, root);
}

// Vote weight of an account
nano::uint128_t nano::ledger::weight (nano::account const & account_a)
{
	nano::amount result;
	rsnano::rsn_ledger_weight (handle, account_a.bytes.data (), result.bytes.data ());
	return result.number ();
}

// Rollback blocks until `block_a' doesn't exist or it tries to penetrate the confirmation height
bool nano::ledger::rollback (store::write_transaction const & transaction_a, nano::block_hash const & block_a, std::vector<std::shared_ptr<nano::block>> & list_a)
{
	rsnano::BlockArrayDto list_dto;
	auto error = rsnano::rsn_ledger_rollback (handle, transaction_a.get_rust_handle (), block_a.bytes.data (), &list_dto);
	rsnano::read_block_array_dto (list_dto, list_a);
	return error;
}

bool nano::ledger::rollback (store::write_transaction const & transaction_a, nano::block_hash const & block_a)
{
	std::vector<std::shared_ptr<nano::block>> rollback_list;
	return rollback (transaction_a, block_a, rollback_list);
}

std::optional<nano::account> nano::ledger::account (store::transaction const & transaction, nano::block_hash const & hash) const
{
	nano::account result;
	bool found = rsnano::rsn_ledger_account (handle, transaction.get_rust_handle (), hash.bytes.data (), result.bytes.data ());
	if (found)
	{
		return result;
	}
	else
	{
		return std::nullopt;
	}
}

std::optional<nano::account_info> nano::ledger::account_info (store::transaction const & transaction, nano::account const & account) const
{
	return store.account ().get (transaction, account);
}

std::optional<nano::uint128_t> nano::ledger::amount (store::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	nano::amount result;
	bool found = rsnano::rsn_ledger_amount (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	if (found)
	{
		return result.number ();
	}
	else
	{
		return std::nullopt;
	}
}

// Return latest block for account
nano::block_hash nano::ledger::latest (store::transaction const & transaction_a, nano::account const & account_a)
{
	nano::block_hash latest_l;
	rsnano::rsn_ledger_latest (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), latest_l.bytes.data ());
	return latest_l;
}

// Return latest root for account, account number if there are no blocks for this account.
nano::root nano::ledger::latest_root (store::transaction const & transaction_a, nano::account const & account_a)
{
	nano::root latest_l;
	rsnano::rsn_ledger_latest_root (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), latest_l.bytes.data ());
	return latest_l;
}

bool nano::ledger::dependents_confirmed (store::transaction const & transaction_a, nano::block const & block_a) const
{
	return rsnano::rsn_ledger_dependents_confirmed (handle, transaction_a.get_rust_handle (), block_a.get_handle ());
}

bool nano::ledger::is_epoch_link (nano::link const & link_a) const
{
	return rsnano::rsn_ledger_is_epoch_link (handle, link_a.bytes.data ());
}

std::array<nano::block_hash, 2> nano::ledger::dependent_blocks (store::transaction const & transaction_a, nano::block const & block_a) const
{
	std::array<nano::block_hash, 2> result;
	rsnano::rsn_ledger_dependent_blocks (handle, transaction_a.get_rust_handle (), block_a.get_handle (), result[0].bytes.data (), result[1].bytes.data ());
	return result;
}

/** Given the block hash of a send block, find the associated receive block that receives that send.
 *  The send block hash is not checked in any way, it is assumed to be correct.
 * @return Return the receive block on success and null on failure
 */
std::shared_ptr<nano::block> nano::ledger::find_receive_block_by_send_hash (store::transaction const & transaction, nano::account const & destination, nano::block_hash const & send_block_hash)
{
	auto block_handle = rsnano::rsn_ledger_find_receive_block_by_send_hash (handle, transaction.get_rust_handle (), destination.bytes.data (), send_block_hash.bytes.data ());
	return nano::block_handle_to_block (block_handle);
}

nano::account nano::ledger::epoch_signer (nano::link const & link_a) const
{
	nano::account signer;
	rsnano::rsn_ledger_epoch_signer (handle, link_a.bytes.data (), signer.bytes.data ());
	return signer;
}

nano::link nano::ledger::epoch_link (nano::epoch epoch_a) const
{
	nano::link link;
	rsnano::rsn_ledger_epoch_link (handle, static_cast<uint8_t> (epoch_a), link.bytes.data ());
	return link;
}

void nano::ledger::update_account (store::write_transaction const & transaction_a, nano::account const & account_a, nano::account_info const & old_a, nano::account_info const & new_a)
{
	rsnano::rsn_ledger_update_account (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), old_a.handle, new_a.handle);
}

std::shared_ptr<nano::block> nano::ledger::successor (store::transaction const & transaction_a, nano::qualified_root const & root_a)
{
	auto block_handle = rsnano::rsn_ledger_successor (handle, transaction_a.get_rust_handle (), root_a.bytes.data ());
	return nano::block_handle_to_block (block_handle);
}

std::shared_ptr<nano::block> nano::ledger::head_block (store::transaction const & transaction, nano::account const & account)
{
	auto info = store.account ().get (transaction, account);
	if (info)
	{
		return store.block ().get (transaction, info->head ());
	}
	return nullptr;
}

bool nano::ledger::block_confirmed (store::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_confirmed (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

uint64_t nano::ledger::pruning_action (store::write_transaction & transaction_a, nano::block_hash const & hash_a, uint64_t const batch_size_a)
{
	return rsnano::rsn_ledger_pruning_action (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), batch_size_a);
}

std::multimap<uint64_t, nano::uncemented_info, std::greater<>> nano::ledger::unconfirmed_frontiers () const
{
	rsnano::UnconfirmedFrontierArrayDto array_dto;
	rsnano::rsn_ledger_unconfirmed_frontiers (handle, &array_dto);
	std::multimap<uint64_t, nano::uncemented_info, std::greater<>> result;
	for (int i = 0; i < array_dto.count; ++i)
	{
		const auto & item_dto = array_dto.items[i].info;
		nano::block_hash cemented_frontier;
		nano::block_hash frontier;
		nano::account account;
		std::copy (std::begin (item_dto.cemented_frontier), std::end (item_dto.cemented_frontier), std::begin (cemented_frontier.bytes));
		std::copy (std::begin (item_dto.frontier), std::end (item_dto.frontier), std::begin (frontier.bytes));
		std::copy (std::begin (item_dto.account), std::end (item_dto.account), std::begin (account.bytes));
		result.emplace (std::piecewise_construct, std::forward_as_tuple (array_dto.items[i].height_delta), std::forward_as_tuple (cemented_frontier, frontier, account));
	}
	rsnano::rsn_unconfirmed_frontiers_destroy (&array_dto);
	return result;
}

bool nano::ledger::bootstrap_weight_reached () const
{
	return rsnano::rsn_ledger_bootstrap_weight_reached (handle);
}

size_t nano::ledger::get_bootstrap_weights_size () const
{
	return get_bootstrap_weights ().size ();
}

void nano::ledger::enable_pruning ()
{
	rsnano::rsn_ledger_enable_pruning (handle);
}

bool nano::ledger::pruning_enabled () const
{
	return rsnano::rsn_ledger_pruning_enabled (handle);
}

std::unordered_map<nano::account, nano::uint128_t> nano::ledger::get_bootstrap_weights () const
{
	std::unordered_map<nano::account, nano::uint128_t> weights;
	rsnano::BootstrapWeightsDto dto;
	rsnano::rsn_ledger_bootstrap_weights (handle, &dto);
	for (int i = 0; i < dto.count; ++i)
	{
		nano::account account;
		nano::uint128_t amount;
		auto & item = dto.accounts[i];
		std::copy (std::begin (item.account), std::end (item.account), std::begin (account.bytes));
		boost::multiprecision::import_bits (amount, std::begin (item.weight), std::end (item.weight), 8, true);
		weights.emplace (account, amount);
	}
	rsnano::rsn_ledger_destroy_bootstrap_weights_dto (&dto);
	return weights;
}

void nano::ledger::set_bootstrap_weights (std::unordered_map<nano::account, nano::uint128_t> const & weights_a)
{
	std::vector<rsnano::BootstrapWeightsItem> dtos;
	dtos.reserve (weights_a.size ());
	for (auto & it : weights_a)
	{
		rsnano::BootstrapWeightsItem dto;
		std::copy (std::begin (it.first.bytes), std::end (it.first.bytes), std::begin (dto.account));
		std::fill (std::begin (dto.weight), std::end (dto.weight), 0);
		boost::multiprecision::export_bits (it.second, std::rbegin (dto.weight), 8, false);
		dtos.push_back (dto);
	}
	rsnano::rsn_ledger_set_bootstrap_weights (handle, dtos.data (), dtos.size ());
}

uint64_t nano::ledger::get_bootstrap_weight_max_blocks () const
{
	return rsnano::rsn_ledger_bootstrap_weight_max_blocks (handle);
}

void nano::ledger::set_bootstrap_weight_max_blocks (uint64_t max_a)
{
	rsnano::rsn_ledger_set_bootstrap_weight_max_blocks (handle, max_a);
}

nano::epoch nano::ledger::version (nano::block const & block)
{
	if (block.type () == nano::block_type::state)
	{
		return block.sideband ().details ().epoch ();
	}

	return nano::epoch::epoch_0;
}

nano::epoch nano::ledger::version (store::transaction const & transaction, nano::block_hash const & hash) const
{
	auto epoch = rsnano::rsn_ledger_version (handle, transaction.get_rust_handle (), hash.bytes.data ());
	return static_cast<nano::epoch> (epoch);
}

uint64_t nano::ledger::height (store::transaction const & transaction, nano::block_hash const & hash) const
{
	return rsnano::rsn_ledger_account_height (handle, transaction.get_rust_handle (), hash.bytes.data ());
}

nano::uncemented_info::uncemented_info (nano::block_hash const & cemented_frontier, nano::block_hash const & frontier, nano::account const & account) :
	cemented_frontier (cemented_frontier), frontier (frontier), account (account)
{
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (ledger & ledger, std::string const & name)
{
	auto count = ledger.get_bootstrap_weights_size ();
	auto sizeof_element = sizeof (nano::account) + sizeof (nano::uint128_t);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "bootstrap_weights", count, sizeof_element }));
	composite->add_component (collect_container_info (ledger.cache.rep_weights (), "rep_weights"));
	return composite;
}
