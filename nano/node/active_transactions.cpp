#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/lib/utility.hpp"

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

#include <chrono>
#include <cstdint>
#include <memory>
#include <stdexcept>

using namespace std::chrono;

nano::active_transactions_lock::active_transactions_lock (nano::active_transactions const & active_transactions) :
	handle{ rsnano::rsn_active_transactions_lock (active_transactions.handle) },
	active_transactions{ active_transactions }
{
}

nano::active_transactions_lock::~active_transactions_lock ()
{
	rsnano::rsn_active_transactions_lock_destroy (handle);
}

void nano::active_transactions_lock::lock ()
{
	rsnano::rsn_active_transactions_lock_lock (handle, active_transactions.handle);
}

void nano::active_transactions_lock::unlock ()
{
	rsnano::rsn_active_transactions_lock_unlock (handle);
}

bool nano::active_transactions_lock::owns_lock ()
{
	return rsnano::rsn_active_transactions_lock_owns_lock (handle);
}

namespace
{
class election_winner_details_lock
{
public:
	election_winner_details_lock (rsnano::ElectionWinnerDetailsLock * handle) :
		handle{ handle }
	{
	}
	election_winner_details_lock (election_winner_details_lock const &) = delete;
	election_winner_details_lock (election_winner_details_lock && other) :
		handle{ other.handle }
	{
		other.handle = nullptr;
	}
	~election_winner_details_lock ()
	{
		if (handle != nullptr)
		{
			rsnano::rsn_election_winner_details_lock_destroy (handle);
		}
	}

	void ensure_locked () const
	{
		if (handle == nullptr)
			throw std::runtime_error ("election_winner_details_lock is unlocked!");
	}

	void unlock ()
	{
		ensure_locked ();
		rsnano::rsn_election_winner_details_lock_unlock (handle);
	}

	std::size_t size () const
	{
		ensure_locked ();
		return rsnano::rsn_election_winner_details_len (handle);
	}

	bool contains (nano::block_hash const & hash) const
	{
		ensure_locked ();
		return rsnano::rsn_election_winner_details_contains (handle, hash.bytes.data ());
	}

	void insert (nano::block_hash const & hash, nano::election const & election)
	{
		ensure_locked ();
		rsnano::rsn_election_winner_details_insert (handle, hash.bytes.data (), election.handle);
	}

	rsnano::ElectionWinnerDetailsLock * handle;
};

election_winner_details_lock lock_election_winners (rsnano::ActiveTransactionsHandle * handle)
{
	return election_winner_details_lock{ rsnano::rsn_active_transactions_lock_election_winner_details (handle) };
}

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
}

nano::active_transactions::active_transactions (nano::node & node_a, nano::confirming_set & confirming_set, nano::block_processor & block_processor_a) :
	node{ node_a },
	confirming_set{ confirming_set },
	block_processor{ block_processor_a },
	recently_cemented{ node.config->confirmation_history_size },
	election_time_to_live{ node_a.network_params.network.is_dev_network () ? 0s : 2s }
{
	auto network_dto{ node_a.network_params.to_dto () };
	auto config_dto{ node_a.config->to_dto () };
	auto observers_context = new std::shared_ptr<nano::node_observers> (node_a.observers);
	handle = rsnano::rsn_active_transactions_create (&network_dto, node_a.online_reps.get_handle (),
	node_a.wallets.rust_handle, &config_dto, node_a.ledger.handle, node_a.confirming_set.handle,
	node_a.workers->handle, node_a.history.handle, node_a.block_processor.handle,
	node_a.generator.handle, node_a.final_generator.handle, node_a.network->tcp_channels->handle,
	node_a.vote_cache.handle, node_a.stats->handle, observers_context, delete_observers_context,
	call_active_stopped);

	// Register a callback which will get called after a block is cemented
	confirming_set.add_cemented_observer ([this] (std::shared_ptr<nano::block> const & callback_block_a) {
		this->block_cemented_callback (callback_block_a);
	});

	// Register a callback which will get called if a block is already cemented
	confirming_set.add_block_already_cemented_observer ([this] (nano::block_hash const & hash_a) {
		this->block_already_cemented_callback (hash_a);
	});

	// Notify elections about alternative (forked) blocks
	block_processor.add_block_processed_observer ([this] (auto const result, auto const & block, auto source) {
		switch (result)
		{
			case nano::block_status::fork:
				publish (block);
				break;
			default:
				break;
		}
	});
}

nano::active_transactions::~active_transactions ()
{
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
	rsnano::rsn_active_transactions_destroy (handle);
}

void nano::active_transactions::start ()
{
	if (node.flags.disable_request_loop ())
	{
		return;
	}

	debug_assert (!thread.joinable ());

	thread = std::thread ([this] () {
		nano::thread_role::set (nano::thread_role::name::request_loop);
		request_loop ();
	});
}

void nano::active_transactions::stop ()
{
	{
		auto guard{ lock () };
		rsnano::rsn_active_transactions_lock_stop (guard.handle);
	}
	rsnano::rsn_active_transactions_notify_all (handle);
	nano::join_or_pass (thread);
	clear ();
}

void nano::active_transactions::block_cemented_callback (std::shared_ptr<nano::block> const & block)
{
	debug_assert (node.block_confirmed (block->hash ()));
	if (auto election_l = election (block->qualified_root ()))
	{
		try_confirm (*election_l, block->hash ());
	}
	auto election = remove_election_winner_details (block->hash ());
	nano::election_status status;
	std::vector<nano::vote_with_weight_info> votes;
	status.set_winner (block);
	if (election)
	{
		status = election->get_status ();
		votes = votes_with_weight (*election);
	}
	if (confirming_set.exists (block->hash ()))
	{
		status.set_election_status_type (nano::election_status_type::active_confirmed_quorum);
	}
	else if (election)
	{
		status.set_election_status_type (nano::election_status_type::active_confirmation_height);
	}
	else
	{
		status.set_election_status_type (nano::election_status_type::inactive_confirmation_height);
	}
	recently_cemented.put (status);
	auto transaction = node.store.tx_begin_read ();
	notify_observers (*transaction, status, votes);
	bool cemented_bootstrap_count_reached = node.ledger.cemented_count () >= node.ledger.get_bootstrap_weight_max_blocks ();
	bool was_active = status.get_election_status_type () == nano::election_status_type::active_confirmed_quorum || status.get_election_status_type () == nano::election_status_type::active_confirmation_height;

	// Next-block activations are only done for blocks with previously active elections
	if (cemented_bootstrap_count_reached && was_active)
	{
		// TODO Gustav: Use callback style?
		// block_cemented_with_active_election(transaction, block);
		node.scheduler.priority.activate_successors (*transaction, block);
	}
}

bool nano::active_transactions::confirmed (nano::election const & election) const
{
	auto guard{ election.lock () };
	return confirmed_locked (guard);
}

bool nano::active_transactions::confirmed_locked (nano::election_lock & lock) const
{
	return rsnano::rsn_active_transactions_confirmed_locked (handle, lock.handle);
}

bool nano::active_transactions::confirmed (nano::block_hash const & hash) const
{
	auto transaction (node.store.tx_begin_read ());
	return node.ledger.block_confirmed (*transaction, hash);
}

void nano::active_transactions::remove_block (nano::election_lock & lock, nano::block_hash const & hash_a)
{
	rsnano::rsn_active_transactions_remove_block (handle, lock.handle, hash_a.bytes.data ());
}

bool nano::active_transactions::replace_by_weight (nano::election & election, nano::election_lock & lock_a, nano::block_hash const & hash_a)
{
	return rsnano::rsn_active_transactions_replace_by_weight (handle, election.handle, lock_a.handle, hash_a.bytes.data ());
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
	}
	result.reserve (sorted_votes.size ());
	std::transform (sorted_votes.begin (), sorted_votes.end (), std::back_inserter (result), [] (auto const & entry) { return entry.second; });
	return result;
}

bool nano::active_transactions::publish (std::shared_ptr<nano::block> const & block_a, nano::election & election)
{
	return rsnano::rsn_active_transactions_publish (handle, block_a->get_handle (), election.handle);
}

void nano::active_transactions::broadcast_vote_locked (nano::election_lock & lock, nano::election & election)
{
	rsnano::rsn_active_transactions_broadcast_vote_locked (handle, lock.handle, election.handle);
}

void nano::active_transactions::broadcast_vote (nano::election & election, nano::election_lock & lock_a)
{
	rsnano::rsn_active_transactions_broadcast_vote (handle, election.handle, lock_a.handle);
}

void nano::active_transactions::notify_observers (nano::store::read_transaction const & transaction, nano::election_status const & status, std::vector<nano::vote_with_weight_info> const & votes)
{
	auto block = status.get_winner ();
	auto account = block->account ();
	auto amount = node.ledger.amount (transaction, block->hash ()).value_or (0);
	auto is_state_send = block->type () == block_type::state && block->is_send ();
	auto is_state_epoch = block->type () == block_type::state && block->is_epoch ();
	node.observers->blocks.notify (status, votes, account, amount, is_state_send, is_state_epoch);

	if (amount > 0)
	{
		node.observers->account_balance.notify (account, false);
		if (block->is_send ())
		{
			node.observers->account_balance.notify (block->destination (), true);
		}
	}
}

void nano::active_transactions::add_election_winner_details (nano::block_hash const & hash_a, std::shared_ptr<nano::election> const & election_a)
{
	auto guard{ lock_election_winners (handle) };
	guard.insert (hash_a, *election_a);
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

nano::active_transactions_lock nano::active_transactions::lock () const
{
	return nano::active_transactions_lock{ *this };
}

void nano::active_transactions::process_confirmed (nano::election_status const & status_a, uint64_t iteration_a)
{
	rsnano::rsn_active_transactions_process_confirmed (handle, status_a.handle, iteration_a);
}

void nano::active_transactions::confirm_once (nano::election_lock & lock_a, nano::election & election)
{
	rsnano::rsn_active_transactions_confirm_once (handle, lock_a.handle, election.handle);
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

bool nano::active_transactions::have_quorum (nano::tally_t const & tally_a) const
{
	auto tally_handle = rsnano::rsn_tally_blocks_create ();
	for (const auto & [weight, block] : tally_a)
	{
		nano::amount amount{ weight };
		rsnano::rsn_tally_blocks_insert (tally_handle, amount.bytes.data (), block->get_handle ());
	}
	bool result = rsnano::rsn_active_transactions_have_quorum (handle, tally_handle);
	rsnano::rsn_tally_blocks_destroy (tally_handle);
	return result;
}

void nano::active_transactions::confirm_if_quorum (nano::election_lock & lock_a, nano::election & election)
{
	rsnano::rsn_active_transactions_confirm_if_quorum (handle, lock_a.handle, election.handle);
}

void nano::active_transactions::force_confirm (nano::election & election)
{
	rsnano::rsn_active_transactions_force_confirm (handle, election.handle);
}

std::chrono::seconds nano::active_transactions::cooldown_time (nano::uint128_t weight) const
{
	nano::amount weight_amount{ weight };
	return std::chrono::seconds{ rsnano::rsn_active_transactions_cooldown_time_s (handle, weight_amount.bytes.data ()) };
}

void nano::active_transactions::block_already_cemented_callback (nano::block_hash const & hash_a)
{
	// Depending on timing there is a situation where the election_winner_details is not reset.
	// This can happen when a block wins an election, and the block is confirmed + observer
	// called before the block hash gets added to election_winner_details. If the block is confirmed
	// callbacks have already been done, so we can safely just remove it.
	remove_election_winner_details (hash_a);
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

void nano::active_transactions::request_confirm (nano::active_transactions_lock & lock_a)
{
	debug_assert (lock_a.owns_lock ());

	std::size_t const this_loop_target_l (rsnano::rsn_active_transactions_lock_roots_size (lock_a.handle));

	auto const elections_l{ list_active_impl (this_loop_target_l, lock_a) };

	lock_a.unlock ();

	nano::confirmation_solicitor solicitor (*node.network, *node.config);
	solicitor.prepare (node.representative_register.principal_representatives (std::numeric_limits<std::size_t>::max ()));

	std::size_t unconfirmed_count_l (0);
	nano::timer<std::chrono::milliseconds> elapsed (nano::timer_state::started);

	/*
	 * Loop through active elections in descending order of proof-of-work difficulty, requesting confirmation
	 *
	 * Only up to a certain amount of elections are queued for confirmation request and block rebroadcasting. The remaining elections can still be confirmed if votes arrive
	 * Elections extending the soft config.active_elections_size limit are flushed after a certain time-to-live cutoff
	 * Flushed elections are later re-activated via frontier confirmation
	 */
	for (auto const & election_l : elections_l)
	{
		bool const confirmed_l (confirmed (*election_l));
		unconfirmed_count_l += !confirmed_l;

		if (confirmed_l || transition_time (solicitor, *election_l))
		{
			erase (election_l->qualified_root ());
		}
	}

	solicitor.flush ();
	lock_a.lock ();
}

void nano::active_transactions::cleanup_election (nano::active_transactions_lock & lock_a, std::shared_ptr<nano::election> election)
{
	rsnano::rsn_active_transactions_cleanup_election (handle, lock_a.handle, election->handle);
}

std::vector<std::shared_ptr<nano::election>> nano::active_transactions::list_active (std::size_t max_a)
{
	auto guard{ lock () };
	return list_active_impl (max_a, guard);
}

std::vector<std::shared_ptr<nano::election>> nano::active_transactions::list_active_impl (std::size_t max_a, nano::active_transactions_lock & guard) const
{
	std::vector<std::shared_ptr<nano::election>> result_l;
	auto elections_handle = rsnano::rsn_active_transactions_lock_roots_get_elections (guard.handle);
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

void nano::active_transactions::request_loop ()
{
	auto guard{ lock () };
	while (!rsnano::rsn_active_transactions_lock_stopped (guard.handle))
	{
		rsnano::instant stamp_l;

		node.stats->inc (nano::stat::type::active, nano::stat::detail::loop);

		request_confirm (guard);

		rsnano::rsn_active_transactions_request_loop (handle, guard.handle, stamp_l.handle);
	}
}

nano::election_insertion_result nano::active_transactions::insert (const std::shared_ptr<nano::block> & block_a, nano::election_behavior election_behavior_a)
{
	debug_assert (block_a);
	debug_assert (block_a->has_sideband ());

	auto guard{ lock () };

	nano::election_insertion_result result;

	if (rsnano::rsn_active_transactions_lock_stopped (guard.handle))
	{
		return result;
	}

	auto const root (block_a->qualified_root ());
	auto const hash = block_a->hash ();
	auto const existing_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
	std::shared_ptr<nano::election> existing{};
	if (existing_handle != nullptr)
	{
		existing = std::make_shared<nano::election> (existing_handle);
	}

	if (existing == nullptr)
	{
		if (!recently_confirmed ().exists (root))
		{
			result.inserted = true;
			auto observe_rep_cb = [&node = node] (auto const & rep_a) {
				// Representative is defined as online if replying to live votes or rep_crawler queries
				node.online_reps.observe (rep_a);
			};
			auto hash (block_a->hash ());
			result.election = nano::make_shared<nano::election> (node, block_a, nullptr, observe_rep_cb, election_behavior_a);
			rsnano::rsn_active_transactions_lock_roots_insert (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data (), result.election->handle);
			rsnano::rsn_active_transactions_lock_blocks_insert (guard.handle, hash.bytes.data (), result.election->handle);

			// Keep track of election count by election type
			debug_assert (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (result.election->behavior ())) >= 0);
			rsnano::rsn_active_transactions_lock_count_by_behavior_inc (guard.handle, static_cast<uint8_t> (result.election->behavior ()));

			node.stats->inc (nano::stat::type::active_started, to_stat_detail (election_behavior_a));
			node.logger->trace (nano::log::type::active_transactions, nano::log::detail::active_started,
			nano::log::arg{ "behavior", election_behavior_a },
			nano::log::arg{ "election", result.election });
		}
		else
		{
			// result is not set
		}
	}
	else
	{
		result.election = existing;
	}
	guard.unlock ();

	if (result.inserted)
	{
		debug_assert (result.election);

		trigger_vote_cache (hash);

		node.observers->active_started.notify (hash);
		vacancy_update ();
	}

	// Votes are generated for inserted or ongoing elections
	if (result.election)
	{
		auto guard{ result.election->lock () };
		broadcast_vote (*result.election, guard);
	}

	trim ();

	return result;
}

bool nano::active_transactions::trigger_vote_cache (nano::block_hash hash)
{
	auto cached = node.vote_cache.find (hash);
	for (auto const & cached_vote : cached)
	{
		vote (cached_vote, nano::vote_source::cache);
	}
	return !cached.empty ();
}

void nano::active_transactions::trim ()
{
	rsnano::rsn_active_transactions_trim (handle);
}

std::chrono::milliseconds nano::active_transactions::base_latency () const
{
	return node.network_params.network.is_dev_network () ? 25ms : 1000ms;
}

std::chrono::milliseconds nano::active_transactions::confirm_req_time (nano::election & election) const
{
	return std::chrono::milliseconds{ rsnano::rsn_active_transactions_confirm_req_time_ms (handle, election.handle) };
}

void nano::active_transactions::send_confirm_req (nano::confirmation_solicitor & solicitor_a, nano::election & election, nano::election_lock & lock_a)
{
	if (confirm_req_time (election) < std::chrono::milliseconds{ rsnano::rsn_election_last_req_elapsed_ms (election.handle) })
	{
		if (!solicitor_a.add (election, lock_a))
		{
			rsnano::rsn_election_last_req_set (election.handle);
			election.inc_confirmation_request_count ();
		}
	}
}

bool nano::active_transactions::transition_time (nano::confirmation_solicitor & solicitor_a, nano::election & election)
{
	auto lock{ election.lock () };
	bool result = false;
	auto state_l = static_cast<nano::election_state> (rsnano::rsn_election_lock_state (lock.handle));
	switch (state_l)
	{
		case nano::election_state::passive:
			if (base_latency () * election.passive_duration_factor < std::chrono::milliseconds{ rsnano::rsn_election_lock_state_start_elapsed_ms (lock.handle) })
			{
				lock.state_change (nano::election_state::passive, nano::election_state::active);
			}
			break;
		case nano::election_state::active:
			broadcast_vote (election, lock);
			broadcast_block (solicitor_a, election, lock);
			send_confirm_req (solicitor_a, election, lock);
			break;
		case nano::election_state::confirmed:
			result = true; // Return true to indicate this election should be cleaned up
			broadcast_block (solicitor_a, election, lock); // Ensure election winner is broadcasted
			lock.state_change (nano::election_state::confirmed, nano::election_state::expired_confirmed);
			break;
		case nano::election_state::expired_unconfirmed:
		case nano::election_state::expired_confirmed:
			debug_assert (false);
			break;
	}

	if (!confirmed_locked (lock) && election.time_to_live () < std::chrono::milliseconds{ rsnano::rsn_election_elapsed_ms (election.handle) })
	{
		// It is possible the election confirmed while acquiring the mutex
		// state_change returning true would indicate it
		state_l = static_cast<nano::election_state> (rsnano::rsn_election_lock_state (lock.handle));
		if (!lock.state_change (state_l, nano::election_state::expired_unconfirmed))
		{
			node.logger->trace (nano::log::type::election, nano::log::detail::election_expired,
			nano::log::arg{ "qualified_root", election.qualified_root () });

			result = true; // Return true to indicate this election should be cleaned up
			auto st{ lock.status () };
			st.set_election_status_type (nano::election_status_type::stopped);
			lock.set_status (st);
		}
	}
	return result;
}

nano::recently_confirmed_cache nano::active_transactions::recently_confirmed ()
{
	return nano::recently_confirmed_cache{ rsnano::rsn_active_transactions_recently_confirmed (handle) };
}

void nano::active_transactions::on_block_confirmed (std::function<void (std::shared_ptr<nano::block> const &, nano::store::read_transaction const &, nano::election_status_type)> callback)
{
	block_confirmed_callback = std::move (callback);
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

bool nano::active_transactions::broadcast_block_predicate (nano::election & election, nano::election_lock & lock_a) const
{
	return rsnano::rsn_active_transactions_broadcast_block_predicate (handle, election.handle, lock_a.handle);
}

void nano::active_transactions::broadcast_block (nano::confirmation_solicitor & solicitor_a, nano::election & election, nano::election_lock & lock_a)
{
	if (broadcast_block_predicate (election, lock_a))
	{
		if (!solicitor_a.broadcast (election, lock_a))
		{
			nano::block_hash last_block_hash{};
			rsnano::rsn_election_lock_last_block (lock_a.handle, last_block_hash.bytes.data ());
			node.stats->inc (nano::stat::type::election, last_block_hash.is_zero () ? nano::stat::detail::broadcast_block_initial : nano::stat::detail::broadcast_block_repeat);
			rsnano::rsn_election_set_last_block (election.handle);
			rsnano::rsn_election_lock_last_block_set (lock_a.handle, lock_a.status ().get_winner ()->hash ().bytes.data ());
		}
	}
}

// Validate a vote and apply it to the current election if one exists
std::unordered_map<nano::block_hash, nano::vote_code> nano::active_transactions::vote (std::shared_ptr<nano::vote> const & vote, nano::vote_source source)
{
	std::unordered_map<nano::block_hash, nano::vote_code> results;
	std::unordered_map<nano::block_hash, std::shared_ptr<nano::election>> process;
	std::vector<nano::block_hash> inactive; // Hashes that should be added to inactive vote cache

	{
		auto guard{ lock () };
		for (auto const & hash : vote->hashes ())
		{
			// Ignore duplicate hashes (should not happen with a well-behaved voting node)
			if (results.find (hash) != results.end ())
			{
				continue;
			}

			auto existing_handle = rsnano::rsn_active_transactions_lock_blocks_find (guard.handle, hash.bytes.data ());
			if (existing_handle != nullptr)
			{
				auto existing = std::make_shared<nano::election> (existing_handle);
				process[hash] = existing;
			}
			else if (!recently_confirmed ().exists (hash))
			{
				inactive.emplace_back (hash);
				results[hash] = nano::vote_code::indeterminate;
			}
			else
			{
				results[hash] = nano::vote_code::replay;
			}
		}
	}

	for (auto const & [block_hash, election] : process)
	{
		auto const vote_result = this->vote (*election, vote->account (), vote->timestamp (), block_hash, source);
		results[block_hash] = vote_result;
	}

	vote_processed.notify (vote, source, results);

	return results;
}

bool nano::active_transactions::active (nano::qualified_root const & root_a) const
{
	auto guard{ lock () };
	return rsnano::rsn_active_transactions_lock_roots_exists (guard.handle, root_a.root ().bytes.data (), root_a.previous ().bytes.data ());
}

bool nano::active_transactions::active (nano::block const & block_a) const
{
	auto guard{ lock () };
	auto root{ block_a.qualified_root () };
	auto hash{ block_a.hash () };
	auto root_exists = rsnano::rsn_active_transactions_lock_roots_exists (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
	auto existing_handle = rsnano::rsn_active_transactions_lock_blocks_find (guard.handle, hash.bytes.data ());
	bool block_exists = existing_handle != nullptr;
	if (block_exists)
	{
		rsnano::rsn_election_destroy (existing_handle);
	}
	return root_exists && block_exists;
}

bool nano::active_transactions::active (const nano::block_hash & hash) const
{
	return rsnano::rsn_active_transactions_active (handle, hash.bytes.data ());
}

std::shared_ptr<nano::election> nano::active_transactions::election (nano::qualified_root const & root_a) const
{
	std::shared_ptr<nano::election> result;
	auto guard{ lock () };
	auto election_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root_a.root ().bytes.data (), root_a.previous ().bytes.data ());
	if (election_handle != nullptr)
	{
		result = std::make_shared<nano::election> (election_handle);
	}
	return result;
}

std::shared_ptr<nano::block> nano::active_transactions::winner (nano::block_hash const & hash_a) const
{
	std::shared_ptr<nano::block> result;
	auto guard{ lock () };
	auto existing_handle = rsnano::rsn_active_transactions_lock_blocks_find (guard.handle, hash_a.bytes.data ());
	if (existing_handle != nullptr)
	{
		auto election = std::make_shared<nano::election> (existing_handle);
		guard.unlock ();
		result = election->winner ();
	}
	return result;
}

bool nano::active_transactions::erase (nano::block const & block_a)
{
	return erase (block_a.qualified_root ());
}

bool nano::active_transactions::erase (nano::qualified_root const & root_a)
{
	auto guard{ lock () };
	auto election_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root_a.root ().bytes.data (), root_a.previous ().bytes.data ());
	if (election_handle != nullptr)
	{
		auto election = std::make_shared<nano::election> (election_handle);
		cleanup_election (guard, election);
		return true;
	}
	return false;
}

bool nano::active_transactions::erase_hash (nano::block_hash const & hash_a)
{
	auto guard{ lock () };
	[[maybe_unused]] auto erased (rsnano::rsn_active_transactions_lock_blocks_erase (guard.handle, hash_a.bytes.data ()));
	debug_assert (erased);
	return erased;
}

void nano::active_transactions::erase_oldest ()
{
	rsnano::rsn_active_transactions_erase_oldest (handle);
}

bool nano::active_transactions::empty () const
{
	auto guard{ lock () };
	return rsnano::rsn_active_transactions_lock_roots_size (guard.handle) == 0;
}

std::size_t nano::active_transactions::size () const
{
	auto guard{ lock () };
	return rsnano::rsn_active_transactions_lock_roots_size (guard.handle);
}

bool nano::active_transactions::publish (std::shared_ptr<nano::block> const & block_a)
{
	auto guard{ lock () };
	auto root = block_a->qualified_root ();
	auto election_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
	auto result (true);
	if (election_handle != nullptr)
	{
		auto election = std::make_shared<nano::election> (election_handle);
		guard.unlock ();
		result = publish (block_a, *election);
		if (!result)
		{
			guard.lock ();
			rsnano::rsn_active_transactions_lock_blocks_insert (guard.handle, block_a->hash ().bytes.data (), election->handle);
			guard.unlock ();

			trigger_vote_cache (block_a->hash ());

			node.stats->inc (nano::stat::type::active, nano::stat::detail::election_block_conflict);
		}
	}
	return result;
}

nano::vote_code nano::active_transactions::vote (nano::election & election, nano::account const & rep, uint64_t timestamp_a, nano::block_hash const & block_hash_a, nano::vote_source vote_source_a)
{
	auto weight = node.ledger.weight (rep);
	if (!node.network_params.network.is_dev_network () && weight <= node.minimum_principal_weight ())
	{
		return vote_code::indeterminate;
	}

	nano::election_lock lock{ election };

	auto last_vote_l{ lock.find_vote (rep) };
	if (last_vote_l.has_value ())
	{
		if (last_vote_l->get_timestamp () > timestamp_a)
		{
			return vote_code::replay;
		}
		if (last_vote_l->get_timestamp () == timestamp_a && !(last_vote_l->get_hash () < block_hash_a))
		{
			return vote_code::replay;
		}

		auto max_vote = timestamp_a == std::numeric_limits<uint64_t>::max () && last_vote_l->get_timestamp () < timestamp_a;

		bool past_cooldown = true;
		// Only cooldown live votes
		if (vote_source_a == nano::vote_source::live) // Only cooldown live votes
		{
			const auto cooldown = cooldown_time (weight);
			past_cooldown = last_vote_l->get_time () <= std::chrono::system_clock::now () - cooldown;
		}

		if (!max_vote && !past_cooldown)
		{
			return vote_code::ignored;
		}
	}
	lock.insert_or_assign_vote (rep, { timestamp_a, block_hash_a });
	if (vote_source_a == nano::vote_source::live)
	{
		rsnano::rsn_election_live_vote_action (election.handle, rep.bytes.data ());
	}

	node.stats->inc (nano::stat::type::election, vote_source_a == nano::vote_source::live ? nano::stat::detail::vote_new : nano::stat::detail::vote_cached);
	node.logger->trace (nano::log::type::election, nano::log::detail::vote_processed,
	nano::log::arg{ "qualified_root", election.qualified_root () },
	nano::log::arg{ "account", rep },
	nano::log::arg{ "hash", block_hash_a },
	nano::log::arg{ "timestamp", timestamp_a },
	nano::log::arg{ "vote_source", vote_source_a },
	nano::log::arg{ "weight", weight });

	if (!confirmed_locked (lock))
	{
		confirm_if_quorum (lock, election);
	}
	return vote_code::vote;
}

void nano::active_transactions::try_confirm (nano::election & election, nano::block_hash const & hash)
{
	rsnano::rsn_active_transactions_try_confirm (handle, election.handle, hash.bytes.data ());
}

std::size_t nano::active_transactions::election_winner_details_size ()
{
	auto guard{ lock_election_winners (handle) };
	return guard.size ();
}

void nano::active_transactions::clear ()
{
	rsnano::rsn_active_transactions_clear (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (active_transactions & active_transactions, std::string const & name)
{
	auto guard{ active_transactions.lock () };

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "roots", rsnano::rsn_active_transactions_lock_roots_size (guard.handle), sizeof (intptr_t) }));

	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocks", rsnano::rsn_active_transactions_lock_blocks_len (guard.handle), sizeof (intptr_t) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "election_winner_details", active_transactions.election_winner_details_size (), sizeof (nano::block_hash) + sizeof (std::size_t) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "normal", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::normal))), 0 }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "hinted", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::hinted))), 0 }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "optimistic", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::optimistic))), 0 }));

	composite->add_component (active_transactions.recently_confirmed ().collect_container_info ("recently_confirmed"));
	composite->add_component (active_transactions.recently_cemented.collect_container_info ("recently_cemented"));

	return composite;
}

/*
 * class recently_confirmed
 */

nano::recently_confirmed_cache::recently_confirmed_cache (std::size_t max_size_a) :
	handle{ rsnano::rsn_recently_confirmed_cache_create (max_size_a) }
{
}

nano::recently_confirmed_cache::recently_confirmed_cache (rsnano::RecentlyConfirmedCacheHandle * handle) :
	handle{ handle }
{
}

nano::recently_confirmed_cache::recently_confirmed_cache (recently_confirmed_cache && other) :
	handle{ other.handle }
{
	other.handle = nullptr;
}

nano::recently_confirmed_cache::~recently_confirmed_cache ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_recently_confirmed_cache_destroy (handle);
	}
}

void nano::recently_confirmed_cache::put (const nano::qualified_root & root, const nano::block_hash & hash)
{
	rsnano::rsn_recently_confirmed_cache_put (handle, root.bytes.data (), hash.bytes.data ());
}

void nano::recently_confirmed_cache::erase (const nano::block_hash & hash)
{
	rsnano::rsn_recently_confirmed_cache_erase (handle, hash.bytes.data ());
}

void nano::recently_confirmed_cache::clear ()
{
	rsnano::rsn_recently_confirmed_cache_clear (handle);
}

bool nano::recently_confirmed_cache::exists (const nano::block_hash & hash) const
{
	return rsnano::rsn_recently_confirmed_cache_hash_exists (handle, hash.bytes.data ());
}

bool nano::recently_confirmed_cache::exists (const nano::qualified_root & root) const
{
	return rsnano::rsn_recently_confirmed_cache_root_exists (handle, root.bytes.data ());
}

std::size_t nano::recently_confirmed_cache::size () const
{
	return rsnano::rsn_recently_confirmed_cache_len (handle);
}

nano::recently_confirmed_cache::entry_t nano::recently_confirmed_cache::back () const
{
	nano::qualified_root root;
	nano::block_hash hash;
	rsnano::rsn_recently_confirmed_cache_back (handle, root.bytes.data (), hash.bytes.data ());
	return { root, hash };
}

std::unique_ptr<nano::container_info_component> nano::recently_confirmed_cache::collect_container_info (const std::string & name)
{
	return std::make_unique<nano::container_info_composite> (rsnano::rsn_recently_confirmed_cache_collect_container_info (handle, name.c_str ()));
}

/*
 * class recently_cemented
 */

nano::recently_cemented_cache::recently_cemented_cache (std::size_t max_size_a) :
	handle (rsnano::rsn_recently_cemented_cache_create1 (max_size_a))
{
}

nano::recently_cemented_cache::recently_cemented_cache (nano::recently_cemented_cache const & other_a) :
	handle (rsnano::rsn_recently_cemented_cache_clone (other_a.handle))
{
}

nano::recently_cemented_cache::~recently_cemented_cache ()
{
	if (handle != nullptr)
		rsnano::rsn_recently_cemented_cache_destroy (handle);
}

nano::recently_cemented_cache & nano::recently_cemented_cache::operator= (const nano::recently_cemented_cache & other_a)
{
	if (handle != nullptr)
		rsnano::rsn_recently_cemented_cache_destroy (handle);

	handle = rsnano::rsn_recently_cemented_cache_clone (other_a.handle);
	return *this;
}

void nano::recently_cemented_cache::put (const nano::election_status & status)
{
	rsnano::rsn_recently_cemented_cache_put (handle, status.handle);
}

nano::recently_cemented_cache::queue_t nano::recently_cemented_cache::list () const
{
	rsnano::RecentlyCementedCachedDto recently_cemented_cache_dto;
	rsnano::rsn_recently_cemented_cache_list (handle, &recently_cemented_cache_dto);
	nano::recently_cemented_cache::queue_t result;
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

std::size_t nano::recently_cemented_cache::size () const
{
	return rsn_recently_cemented_cache_size (handle);
}

std::unique_ptr<nano::container_info_component> nano::recently_cemented_cache::collect_container_info (const std::string & name)
{
	size_t size = rsnano::rsn_recently_cemented_cache_size (handle);
	size_t size_of_type = rsnano::rsn_recently_cemented_cache_get_cemented_type_size ();

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "cemented", size, size_of_type }));
	return composite;
}
