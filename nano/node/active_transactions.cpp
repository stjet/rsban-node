#include "nano/lib/rsnano.hpp"
#include "nano/lib/utility.hpp"
#include "nano/node/election_status.hpp"
#include "nano/store/lmdb/transaction_impl.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/confirmation_solicitor.hpp>
#include <nano/node/confirming_set.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

#include <boost/format.hpp>

#include <cstdint>
#include <memory>
#include <stdexcept>

using namespace std::chrono;

namespace
{
void call_vacancy_update (void * context)
{
	auto callback = static_cast<std::function<void ()> *> (context);
	(*callback) ();
}

void delete_vacancy_update (void * context)
{
	auto callback = static_cast<std::function<void ()> *> (context);
	delete callback;
}

void call_active_started (void * context, uint8_t const * hash)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	(*observers)->active_started.notify (nano::block_hash::from_bytes (hash));
}

void call_active_stopped (void * context, uint8_t const * hash)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	(*observers)->active_stopped.notify (nano::block_hash::from_bytes (hash));
}

void delete_observers_context (void * context)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	delete observers;
}

void call_vote_processed (void * context, rsnano::VoteHandle * vote_handle, uint8_t source, rsnano::VoteResultMapHandle * results_handle)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::vote> const &, nano::vote_source, std::unordered_map<nano::block_hash, nano::vote_code> const &)> *> (context);
	auto vote = std::make_shared<nano::vote> (vote_handle);
	std::unordered_map<nano::block_hash, nano::vote_code> result;
	auto len = rsnano::rsn_vote_result_map_len (results_handle);
	for (auto i = 0; i < len; ++i)
	{
		nano::block_hash hash;
		auto code = rsnano::rsn_vote_result_map_get (results_handle, i, hash.bytes.data ());
		result.emplace (hash, static_cast<nano::vote_code> (code));
	}
	rsnano::rsn_vote_result_map_destroy (results_handle);
	(*callback) (vote, static_cast<nano::vote_source> (source), result);
}

void delete_vote_processed_context (void * context)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::vote> const &, nano::vote_source, std::unordered_map<nano::block_hash, nano::vote_code> const &)> *> (context);
	delete callback;
}

void call_activate_successors (void * context, rsnano::TransactionHandle * tx_handle, rsnano::BlockHandle * block_handle)
{
	auto callback = static_cast<std::function<void (nano::store::read_transaction const &, std::shared_ptr<nano::block> const &)> *> (context);
	auto block{ nano::block_handle_to_block (block_handle) };
	nano::store::lmdb::read_transaction_impl tx{ tx_handle };
	(*callback) (tx, block);
}

void delete_activate_successors_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::store::read_transaction const &, std::shared_ptr<nano::block> const &)> *> (context);
	delete callback;
}

void call_election_ended (void * context, rsnano::ElectionStatusHandle * status_handle,
rsnano::VoteWithWeightInfoVecHandle * votes_handle, uint8_t const * account_bytes,
uint8_t const * amount_bytes, bool is_state_send, bool is_state_epoch)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);

	nano::election_status status{ status_handle };

	std::vector<nano::vote_with_weight_info> votes;
	auto len = rsnano::rsn_vote_with_weight_info_vec_len (votes_handle);
	for (auto i = 0; i < len; ++i)
	{
		rsnano::VoteWithWeightInfoDto dto;
		rsnano::rsn_vote_with_weight_info_vec_get (votes_handle, i, &dto);
		votes.emplace_back (dto);
	}
	rsnano::rsn_vote_with_weight_info_vec_destroy (votes_handle);

	auto account{ nano::account::from_bytes (account_bytes) };
	auto amount{ nano::amount::from_bytes (amount_bytes) };

	(*observers)->blocks.notify (status, votes, account, amount.number (), is_state_send, is_state_epoch);
}

void call_account_balance_changed (void * context, uint8_t const * account, bool is_pending)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	(*observers)->account_balance.notify (nano::account::from_bytes (account), is_pending);
}

}

nano::active_transactions::active_transactions (nano::node & node_a, nano::confirming_set & confirming_set, nano::block_processor & block_processor_a) :
	node{ node_a }
{
	auto network_dto{ node_a.network_params.to_dto () };
	auto config_dto{ node_a.config->to_dto () };
	auto observers_context = new std::shared_ptr<nano::node_observers> (node_a.observers);

	handle = rsnano::rsn_active_transactions_create (&network_dto, node_a.online_reps.get_handle (),
	node_a.wallets.rust_handle, &config_dto, node_a.ledger.handle, node_a.confirming_set.handle,
	node_a.workers->handle, node_a.history.handle, node_a.block_processor.handle,
	node_a.generator.handle, node_a.final_generator.handle, node_a.network->tcp_channels->handle,
	node_a.vote_cache.handle, node_a.stats->handle, observers_context, delete_observers_context,
	call_active_started, call_active_stopped, call_election_ended, call_account_balance_changed,
	node_a.representative_register.handle, node_a.flags.handle);

	auto activate_successors_context = new std::function<void (nano::store::read_transaction const &, std::shared_ptr<nano::block> const &)>{
		[&node_a] (nano::store::read_transaction const & tx, std::shared_ptr<nano::block> const & block) {
			node_a.scheduler.priority.activate_successors (tx, block);
		}
	};
	rsnano::rsn_active_transactions_activate_successors (handle, call_activate_successors, activate_successors_context, delete_activate_successors_context);
	rsnano::rsn_active_transactions_initialize (handle);
}

nano::active_transactions::~active_transactions ()
{
	rsnano::rsn_active_transactions_destroy (handle);
}

void nano::active_transactions::start ()
{
	rsnano::rsn_active_transactions_start (handle);
}

void nano::active_transactions::stop ()
{
	rsnano::rsn_active_transactions_stop (handle);
}

bool nano::active_transactions::confirmed (nano::election const & election) const
{
	return rsnano::rsn_active_transactions_confirmed (handle, election.handle);
}

bool nano::active_transactions::confirmed (nano::block_hash const & hash) const
{
	auto transaction (node.store.tx_begin_read ());
	return node.ledger.block_confirmed (*transaction, hash);
}

std::vector<nano::vote_with_weight_info> nano::active_transactions::votes_with_weight (nano::election & election) const
{
	std::multimap<nano::uint128_t, nano::vote_with_weight_info, std::greater<nano::uint128_t>> sorted_votes;
	std::vector<nano::vote_with_weight_info> result;
	auto votes_l (election.votes ());
	for (auto const & vote_l : votes_l)
	{
		if (vote_l.first != nullptr)
		{
			auto amount (node.ledger.cache.rep_weights ().representation_get (vote_l.first));
			nano::vote_with_weight_info vote_info{ vote_l.first, vote_l.second.get_time (), vote_l.second.get_timestamp (), vote_l.second.get_hash (), amount };
			sorted_votes.emplace (std::move (amount), vote_info);
		}
		else
		{
		}
	}
	result.reserve (sorted_votes.size ());
	std::transform (sorted_votes.begin (), sorted_votes.end (), std::back_inserter (result), [] (auto const & entry) { return entry.second; });
	return result;
}

bool nano::active_transactions::publish (std::shared_ptr<nano::block> const & block_a, nano::election & election)
{
	return rsnano::rsn_active_transactions_publish (handle, block_a->get_handle (), election.handle);
}

void nano::active_transactions::add_election_winner_details (nano::block_hash const & hash_a, std::shared_ptr<nano::election> const & election_a)
{
	rsnano::rsn_active_transactions_add_election_winner_details (handle, hash_a.bytes.data (), election_a->handle);
}

std::shared_ptr<nano::election> nano::active_transactions::remove_election_winner_details (nano::block_hash const & hash_a)
{
	std::shared_ptr<nano::election> removed{};
	auto election_handle = rsnano::rsn_active_transactions_remove_election_winner_details (handle, hash_a.bytes.data ());
	if (election_handle != nullptr)
	{
		removed = std::make_shared<nano::election> (election_handle);
	}
	return removed;
}

void nano::active_transactions::add_vote_processed_observer (std::function<void (std::shared_ptr<nano::vote> const &, nano::vote_source, std::unordered_map<nano::block_hash, nano::vote_code> const &)> observer)
{
	auto context = new std::function<void (std::shared_ptr<nano::vote> const &, nano::vote_source, std::unordered_map<nano::block_hash, nano::vote_code> const &)> (observer);
	rsnano::rsn_active_transactions_add_vote_processed_observer (handle, call_vote_processed, context, delete_vote_processed_context);
}

void nano::active_transactions::process_confirmed (nano::election_status const & status_a, uint64_t iteration_a)
{
	rsnano::rsn_active_transactions_process_confirmed (handle, status_a.handle, iteration_a);
}

nano::tally_t nano::active_transactions::tally_impl (nano::election_lock & lock) const
{
	nano::tally_t result;
	auto tally_handle = rsnano::rsn_active_transactions_tally_impl (handle, lock.handle);
	for (size_t i = 0, n = rsnano::rsn_tally_blocks_len (tally_handle); i < n; ++i)
	{
		nano::amount weight;
		auto block_handle = rsnano::rsn_tally_blocks_get (tally_handle, i, weight.bytes.data ());
		result.emplace (weight.number (), nano::block_handle_to_block (block_handle));
	}
	rsnano::rsn_tally_blocks_destroy (tally_handle);
	return result;
}

void nano::active_transactions::remove_votes (nano::election & election, nano::election_lock & lock, nano::block_hash const & hash_a)
{
	rsnano::rsn_active_transactions_remove_votes (handle, election.handle, lock.handle, hash_a.bytes.data ());
}

void nano::active_transactions::force_confirm (nano::election & election)
{
	rsnano::rsn_active_transactions_force_confirm (handle, election.handle);
}

int64_t nano::active_transactions::limit (nano::election_behavior behavior) const
{
	return rsnano::rsn_active_transactions_limit (handle, static_cast<uint8_t> (behavior));
}

int64_t nano::active_transactions::vacancy (nano::election_behavior behavior) const
{
	return rsnano::rsn_active_transactions_vacancy (handle, static_cast<uint8_t> (behavior));
}

void nano::active_transactions::set_vacancy_update (std::function<void ()> callback)
{
	auto context = new std::function<void ()> (callback);
	rsnano::rsn_active_transactions_set_vacancy_update (handle, context, call_vacancy_update, delete_vacancy_update);
}

void nano::active_transactions::vacancy_update ()
{
	rsnano::rsn_active_transactions_vacancy_update (handle);
}

std::vector<std::shared_ptr<nano::election>> nano::active_transactions::list_active (std::size_t max_a)
{
	std::vector<std::shared_ptr<nano::election>> result_l;
	auto elections_handle = rsnano::rsn_active_transactions_list_active (handle, max_a);
	auto len = rsnano::rsn_election_vec_len (elections_handle);
	result_l.reserve (std::min (max_a, len));
	std::size_t count_l{ 0 };
	for (auto i = 0; i < len && count_l < max_a; ++i, ++count_l)
	{
		auto election = std::make_shared<nano::election> (rsnano::rsn_election_vec_get (elections_handle, i));
		result_l.push_back (election);
	}
	rsnano::rsn_election_vec_destroy (elections_handle);
	return result_l;
}

nano::election_insertion_result nano::active_transactions::insert (const std::shared_ptr<nano::block> & block_a, nano::election_behavior election_behavior_a)
{
	nano::election_insertion_result result;
	auto election_handle = rsnano::rsn_active_transactions_insert (handle, block_a->get_handle (), static_cast<uint8_t> (election_behavior_a), &result.inserted);
	if (election_handle != nullptr)
	{
		result.election = std::make_shared<nano::election> (election_handle);
	}
	return result;
}

nano::election_extended_status nano::active_transactions::current_status (nano::election & election) const
{
	nano::election_lock guard{ election };
	nano::election_status status_l = guard.status ();
	status_l.set_confirmation_request_count (election.get_confirmation_request_count ());
	status_l.set_block_count (nano::narrow_cast<decltype (status_l.get_block_count ())> (guard.last_blocks_size ()));
	status_l.set_voter_count (nano::narrow_cast<decltype (status_l.get_voter_count ())> (guard.last_votes_size ()));
	return nano::election_extended_status{ status_l, guard.last_votes (), tally_impl (guard) };
}

nano::tally_t nano::active_transactions::tally (nano::election & election) const
{
	auto guard{ election.lock () };
	return tally_impl (guard);
}

void nano::active_transactions::clear_recently_confirmed ()
{
	rsnano::rsn_active_transactions_clear_recently_confirmed (handle);
}

std::size_t nano::active_transactions::recently_confirmed_size ()
{
	return rsnano::rsn_active_transactions_recently_confirmed_count (handle);
}

std::size_t nano::active_transactions::recently_cemented_size ()
{
	return rsnano::rsn_active_transactions_recently_cemented_count (handle);
}

bool nano::active_transactions::recently_confirmed (nano::block_hash const & hash)
{
	return rsnano::rsn_active_transactions_was_recently_confirmed (handle, hash.bytes.data ());
}

nano::qualified_root nano::active_transactions::lastest_recently_confirmed_root ()
{
	nano::qualified_root result;
	rsnano::rsn_active_transactions_latest_recently_confirmed_root (handle, result.bytes.data ());
	return result;
}

void nano::active_transactions::insert_recently_confirmed (std::shared_ptr<nano::block> const & block)
{
	rsnano::rsn_active_transactions_recently_confirmed_insert (handle, block->get_handle ());
}

void nano::active_transactions::insert_recently_cemented (nano::election_status const & status)
{
	rsnano::rsn_active_transactions_recently_cemented_insert (handle, status.handle);
}

std::deque<nano::election_status> nano::active_transactions::recently_cemented_list ()
{
	rsnano::RecentlyCementedCachedDto recently_cemented_cache_dto;
	rsnano::rsn_active_transactions_recently_cemented_list (handle, &recently_cemented_cache_dto);
	std::deque<nano::election_status> result;
	rsnano::ElectionStatusHandle * const * current;
	int i;
	for (i = 0, current = recently_cemented_cache_dto.items; i < recently_cemented_cache_dto.count; ++i)
	{
		nano::election_status election_status (*current);
		result.push_back (election_status);
		current++;
	}

	rsnano::rsn_recently_cemented_cache_destroy_dto (&recently_cemented_cache_dto);

	return result;
}

// Validate a vote and apply it to the current election if one exists
std::unordered_map<nano::block_hash, nano::vote_code> nano::active_transactions::vote (std::shared_ptr<nano::vote> const & vote, nano::vote_source source)
{
	auto result_handle = rsnano::rsn_active_transactions_vote (handle, vote->get_handle (), static_cast<uint8_t> (source));
	std::unordered_map<nano::block_hash, nano::vote_code> result;
	auto len = rsnano::rsn_vote_result_map_len (result_handle);
	for (auto i = 0; i < len; ++i)
	{
		nano::block_hash hash;
		auto code = rsnano::rsn_vote_result_map_get (result_handle, i, hash.bytes.data ());
		result.emplace (hash, static_cast<nano::vote_code> (code));
	}
	rsnano::rsn_vote_result_map_destroy (result_handle);
	return result;
}

bool nano::active_transactions::active (nano::qualified_root const & root_a) const
{
	return rsnano::rsn_active_transactions_active_root (handle, root_a.bytes.data ());
}

bool nano::active_transactions::active (nano::block const & block_a) const
{
	return rsnano::rsn_active_transactions_active (handle, block_a.get_handle ());
}

bool nano::active_transactions::active (const nano::block_hash & hash) const
{
	return rsnano::rsn_active_transactions_active_block (handle, hash.bytes.data ());
}

std::shared_ptr<nano::election> nano::active_transactions::election (nano::qualified_root const & root_a) const
{
	std::shared_ptr<nano::election> result;
	auto election_handle = rsnano::rsn_active_transactions_election (handle, root_a.bytes.data ());
	if (election_handle != nullptr)
	{
		result = std::make_shared<nano::election> (election_handle);
	}
	return result;
}

std::shared_ptr<nano::block> nano::active_transactions::winner (nano::block_hash const & hash_a) const
{
	auto winner_handle = rsnano::rsn_active_transactions_winner (handle, hash_a.bytes.data ());
	return nano::block_handle_to_block (winner_handle);
}

bool nano::active_transactions::erase (nano::block const & block_a)
{
	return erase (block_a.qualified_root ());
}

bool nano::active_transactions::erase (nano::qualified_root const & root_a)
{
	return rsnano::rsn_active_transactions_erase (handle, root_a.bytes.data ());
}

bool nano::active_transactions::empty () const
{
	return size () == 0;
}

std::size_t nano::active_transactions::size () const
{
	return rsnano::rsn_active_transactions_len (handle);
}

bool nano::active_transactions::publish (std::shared_ptr<nano::block> const & block_a)
{
	return rsnano::rsn_active_transactions_publish_block (handle, block_a->get_handle ());
}

nano::vote_code nano::active_transactions::vote (nano::election & election, nano::account const & rep, uint64_t timestamp_a, nano::block_hash const & block_hash_a, nano::vote_source vote_source_a)
{
	auto result = rsnano::rsn_active_transactions_vote2 (handle, election.handle, rep.bytes.data (), timestamp_a, block_hash_a.bytes.data (), static_cast<uint8_t> (vote_source_a));
	return static_cast<nano::vote_code> (result);
}

std::size_t nano::active_transactions::election_winner_details_size ()
{
	return rsnano::rsn_active_transactions_election_winner_details_len (handle);
}

void nano::active_transactions::clear ()
{
	rsnano::rsn_active_transactions_clear (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (active_transactions & active_transactions, std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_active_transactions_collect_container_info (active_transactions.handle, name.c_str ()));
}
