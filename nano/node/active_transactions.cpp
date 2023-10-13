#include "nano/lib/rsnano.hpp"
#include "nano/lib/utility.hpp"

#include <nano/lib/threading.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/confirmation_height_processor.hpp>
#include <nano/node/confirmation_solicitor.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/store/component.hpp>

#include <boost/format.hpp>

#include <cstdint>
#include <memory>

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

nano::active_transactions::active_transactions (nano::node & node_a, nano::confirmation_height_processor & confirmation_height_processor_a) :
	confirmation_height_processor{ confirmation_height_processor_a },
	node{ node_a },
	recently_confirmed{ 65536 },
	recently_cemented{ node.config->confirmation_history_size },
	election_time_to_live{ node_a.network_params.network.is_dev_network () ? 0s : 2s }
{
	auto network_dto{ node_a.network_params.to_dto () };
	handle = rsnano::rsn_active_transactions_create (&network_dto);

	// Register a callback which will get called after a block is cemented
	confirmation_height_processor.set_cemented_observer ([this] (std::shared_ptr<nano::block> const & callback_block_a) {
		this->block_cemented_callback (callback_block_a);
	});

	// Register a callback which will get called if a block is already cemented
	confirmation_height_processor.set_block_already_cemented_observer ([this] (nano::block_hash const & hash_a) {
		this->block_already_cemented_callback (hash_a);
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

void nano::active_transactions::block_cemented_callback (std::shared_ptr<nano::block> const & block_a)
{
	auto transaction = node.store.tx_begin_read ();
	auto status_type = election_status (*transaction, block_a);

	if (!status_type)
		return;

	switch (*status_type)
	{
		case nano::election_status_type::inactive_confirmation_height:
			process_inactive_confirmation (*transaction, block_a);
			break;

		default:
			process_active_confirmation (*transaction, block_a, *status_type);
			break;
	}

	handle_final_votes_confirmation (block_a, *transaction, *status_type);
}

boost::optional<nano::election_status_type> nano::active_transactions::election_status (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block)
{
	boost::optional<nano::election_status_type> status_type;

	if (!confirmation_height_processor.is_processing_added_block (block->hash ()))
	{
		status_type = confirm_block (transaction, block);
	}
	else
	{
		status_type = nano::election_status_type::active_confirmed_quorum;
	}

	return status_type;
}

void nano::active_transactions::process_inactive_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block)
{
	nano::account account;
	nano::uint128_t amount{ 0 };
	bool is_state_send = false;
	bool is_state_epoch = false;
	nano::account pending_account{};
	node.process_confirmed_data (transaction, block, block->hash (), account, amount, is_state_send, is_state_epoch, pending_account);
	nano::election_status status{ block };
	status.set_election_end (std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::system_clock::now ().time_since_epoch ()));
	status.set_block_count (1);
	status.set_election_status_type (nano::election_status_type::inactive_confirmation_height);
	node.observers->blocks.notify (status, {}, account, amount, is_state_send, is_state_epoch);
}

void nano::active_transactions::process_active_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block, nano::election_status_type status_type)
{
	auto hash (block->hash ());
	nano::unique_lock<nano::mutex> election_winners_lk{ election_winner_details_mutex };
	auto existing = election_winner_details.find (hash);
	if (existing != election_winner_details.end ())
	{
		auto election = existing->second;
		election_winner_details.erase (hash);
		election_winners_lk.unlock ();
		if (node.election_helper.confirmed (*election) && election->winner ()->hash () == hash)
		{
			handle_confirmation (transaction, block, election, status_type);
		}
	}
}

void nano::active_transactions::handle_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block, std::shared_ptr<nano::election> election, nano::election_status_type status_type)
{
	nano::block_hash hash = block->hash ();
	update_recently_cemented (election);

	nano::account account;
	nano::uint128_t amount (0);
	bool is_state_send = false;
	bool is_state_epoch = false;
	nano::account pending_account;

	handle_block_confirmation (transaction, block, hash, account, amount, is_state_send, is_state_epoch, pending_account);

	election->set_status_type (status_type);
	notify_observers (election, account, amount, is_state_send, is_state_epoch, pending_account);
}

void nano::active_transactions::update_recently_cemented (std::shared_ptr<nano::election> const & election)
{
	recently_cemented.put (election->get_status ());
}

void nano::active_transactions::handle_block_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block, nano::block_hash const & hash, nano::account & account, nano::uint128_t & amount, bool & is_state_send, bool & is_state_epoch, nano::account & pending_account)
{
	auto destination = block->link ().is_zero () ? block->destination () : block->link ().as_account ();
	node.receive_confirmed (transaction, hash, destination);
	node.process_confirmed_data (transaction, block, hash, account, amount, is_state_send, is_state_epoch, pending_account);
}

void nano::active_transactions::notify_observers (std::shared_ptr<nano::election> const & election, nano::account const & account, nano::uint128_t amount, bool is_state_send, bool is_state_epoch, nano::account const & pending_account)
{
	auto status = election->get_status ();
	auto votes = node.election_helper.votes_with_weight (*election);

	node.observers->blocks.notify (status, votes, account, amount, is_state_send, is_state_epoch);

	if (amount > 0)
	{
		node.observers->account_balance.notify (account, false);
		if (!pending_account.is_zero ())
		{
			node.observers->account_balance.notify (pending_account, true);
		}
	}
}

void nano::active_transactions::handle_final_votes_confirmation (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction, nano::election_status_type status)
{
	auto const & account = !block->account ().is_zero () ? block->account () : block->sideband ().account ();

	bool is_canary_not_set = !node.ledger.cache.final_votes_confirmation_canary ();
	bool is_canary_account = account == node.network_params.ledger.final_votes_canary_account;
	bool is_height_above_threshold = block->sideband ().height () >= node.network_params.ledger.final_votes_canary_height;

	if (is_canary_not_set && is_canary_account && is_height_above_threshold)
	{
		node.ledger.cache.set_final_votes_confirmation_canary (true);
	}

	bool cemented_bootstrap_count_reached = node.ledger.cache.cemented_count () >= node.ledger.get_bootstrap_weight_max_blocks ();
	bool was_active = status == nano::election_status_type::active_confirmed_quorum || status == nano::election_status_type::active_confirmation_height;

	// Next-block activations are only done for blocks with previously active elections
	if (cemented_bootstrap_count_reached && was_active)
	{
		activate_successors (account, block, transaction);
	}
}

void nano::active_transactions::activate_successors (const nano::account & account, std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction)
{
	node.scheduler.priority.activate (account, transaction);
	auto const & destination = node.ledger.block_destination (transaction, *block);

	// Start or vote for the next unconfirmed block in the destination account
	if (!destination.is_zero () && destination != account)
	{
		node.scheduler.priority.activate (destination, transaction);
	}
}

void nano::active_transactions::add_election_winner_details (nano::block_hash const & hash_a, std::shared_ptr<nano::election> const & election_a)
{
	nano::lock_guard<nano::mutex> guard{ election_winner_details_mutex };
	election_winner_details.emplace (hash_a, election_a);
}

void nano::active_transactions::remove_election_winner_details (nano::block_hash const & hash_a)
{
	nano::lock_guard<nano::mutex> guard{ election_winner_details_mutex };
	election_winner_details.erase (hash_a);
}

nano::active_transactions_lock nano::active_transactions::lock () const
{
	return nano::active_transactions_lock{ *this };
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
	switch (behavior)
	{
		case nano::election_behavior::normal:
		{
			return static_cast<int64_t> (node.config->active_elections_size);
		}
		case nano::election_behavior::hinted:
		{
			const uint64_t limit = node.config->active_elections_hinted_limit_percentage * node.config->active_elections_size / 100;
			return static_cast<int64_t> (limit);
		}
		case nano::election_behavior::optimistic:
		{
			const uint64_t limit = node.config->active_elections_optimistic_limit_percentage * node.config->active_elections_size / 100;
			return static_cast<int64_t> (limit);
		}
	}

	debug_assert (false, "unknown election behavior");
	return 0;
}

int64_t nano::active_transactions::vacancy (nano::election_behavior behavior) const
{
	auto guard{ lock () };
	switch (behavior)
	{
		case nano::election_behavior::normal:
			return limit () - static_cast<int64_t> (rsnano::rsn_active_transactions_lock_roots_size (guard.handle));
		case nano::election_behavior::hinted:
		case nano::election_behavior::optimistic:
			return limit (behavior) - rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (behavior));
	}
	debug_assert (false); // Unknown enum
	return 0;
}

void nano::active_transactions::request_confirm (nano::active_transactions_lock & lock_a)
{
	debug_assert (lock_a.owns_lock ());

	std::size_t const this_loop_target_l (rsnano::rsn_active_transactions_lock_roots_size (lock_a.handle));

	auto const elections_l{ list_active_impl (this_loop_target_l, lock_a) };

	lock_a.unlock ();

	nano::confirmation_solicitor solicitor (*node.network, *node.config);
	solicitor.prepare (node.rep_crawler.principal_representatives (std::numeric_limits<std::size_t>::max ()));

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
		bool const confirmed_l (node.election_helper.confirmed (*election_l));
		unconfirmed_count_l += !confirmed_l;

		if (confirmed_l || node.election_helper.transition_time (solicitor, *election_l))
		{
			erase (election_l->qualified_root ());
		}
	}

	solicitor.flush ();
	lock_a.lock ();

	if (node.config->logging.timing_logging ())
	{
		node.logger->try_log (boost::str (boost::format ("Processed %1% elections (%2% were already confirmed) in %3% %4%") % this_loop_target_l % (this_loop_target_l - unconfirmed_count_l) % elapsed.value ().count () % elapsed.unit ()));
	}
}

void nano::active_transactions::cleanup_election (nano::active_transactions_lock & lock_a, std::shared_ptr<nano::election> election)
{
	debug_assert (lock_a.owns_lock ());

	node.stats->inc (completion_type (*election), nano::to_stat_detail (election->behavior ()));
	// Keep track of election count by election type
	debug_assert (rsnano::rsn_active_transactions_lock_count_by_behavior (lock_a.handle, static_cast<uint8_t> (election->behavior ())) > 0);
	rsnano::rsn_active_transactions_lock_count_by_behavior_dec (lock_a.handle, static_cast<uint8_t> (election->behavior ()));

	auto blocks_l = election->blocks ();
	for (auto const & [hash, block] : blocks_l)
	{
		auto erased (blocks.erase (hash));
		(void)erased;
		debug_assert (erased == 1);
		node.inactive_vote_cache.erase (hash);
	}

	auto election_root{ election->qualified_root () };
	rsnano::rsn_active_transactions_lock_roots_erase (lock_a.handle, election_root.root ().bytes.data (), election_root.previous ().bytes.data ());

	lock_a.unlock ();
	vacancy_update ();
	for (auto const & [hash, block] : blocks_l)
	{
		// Notify observers about dropped elections & blocks lost confirmed elections
		if (!node.election_helper.confirmed (*election) || hash != election->winner ()->hash ())
		{
			node.observers->active_stopped.notify (hash);
		}

		if (!node.election_helper.confirmed (*election))
		{
			// Clear from publish filter
			node.network->tcp_channels->publish_filter->clear (block);
		}
	}

	if (node.config->logging.election_result_logging ())
	{
		node.logger->try_log (boost::str (boost::format ("Election erased for root %1%, confirmed: %2$b") % election->qualified_root ().to_string () % node.election_helper.confirmed (*election)));
	}
}

nano::stat::type nano::active_transactions::completion_type (nano::election const & election) const
{
	if (election.status_confirmed ())
	{
		return nano::stat::type::active_confirmed;
	}
	if (election.failed ())
	{
		return nano::stat::type::active_timeout;
	}
	return nano::stat::type::active_dropped;
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

nano::election_insertion_result nano::active_transactions::insert (const std::shared_ptr<nano::block> & block, nano::election_behavior behavior)
{
	debug_assert (block != nullptr);

	auto guard{ lock () };

	auto result = insert_impl (guard, block, behavior);
	return result;
}

void nano::active_transactions::trim ()
{
	/*
	 * Both normal and hinted election schedulers are well-behaved, meaning they first check for AEC vacancy before inserting new elections.
	 * However, it is possible that AEC will be temporarily overfilled in case it's running at full capacity and election hinting or manual queue kicks in.
	 * That case will lead to unwanted churning of elections, so this allows for AEC to be overfilled to 125% until erasing of elections happens.
	 */
	while (vacancy () < -(limit () / 4))
	{
		node.stats->inc (nano::stat::type::active, nano::stat::detail::erase_oldest);
		erase_oldest ();
	}
}

nano::election_insertion_result nano::active_transactions::insert_impl (nano::active_transactions_lock & lock_a, std::shared_ptr<nano::block> const & block_a, nano::election_behavior election_behavior_a, std::function<void (std::shared_ptr<nano::block> const &)> const & confirmation_action_a)
{
	debug_assert (lock_a.owns_lock ());
	debug_assert (block_a->has_sideband ());
	nano::election_insertion_result result;
	if (!rsnano::rsn_active_transactions_lock_stopped (lock_a.handle))
	{
		auto root (block_a->qualified_root ());
		auto existing_handle = rsnano::rsn_active_transactions_lock_roots_find (lock_a.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
		std::shared_ptr<nano::election> existing{};
		if (existing_handle != nullptr)
		{
			existing = std::make_shared<nano::election> (existing_handle);
		}

		if (existing == nullptr)
		{
			if (!recently_confirmed.exists (root))
			{
				result.inserted = true;
				auto hash (block_a->hash ());
				result.election = nano::make_shared<nano::election> (
				node, block_a, confirmation_action_a, [&node = node] (auto const & rep_a) {
					// Representative is defined as online if replying to live votes or rep_crawler queries
					node.online_reps.observe (rep_a);
				},
				election_behavior_a);

				rsnano::rsn_active_transactions_lock_roots_insert (lock_a.handle, root.root ().bytes.data (), root.previous ().bytes.data (), result.election->handle);
				blocks.emplace (hash, result.election);
				// Keep track of election count by election type
				debug_assert (rsnano::rsn_active_transactions_lock_count_by_behavior (lock_a.handle, static_cast<uint8_t> (result.election->behavior ())) >= 0);
				rsnano::rsn_active_transactions_lock_count_by_behavior_inc (lock_a.handle, static_cast<uint8_t> (result.election->behavior ()));
				lock_a.unlock ();
				if (auto const cache = node.inactive_vote_cache.find (hash); cache)
				{
					result.election->fill_from_cache (node.election_helper, *cache);
				}
				node.stats->inc (nano::stat::type::active_started, nano::to_stat_detail (election_behavior_a));
				node.observers->active_started.notify (hash);
				vacancy_update ();
			}
		}
		else
		{
			result.election = existing;
		}

		if (lock_a.owns_lock ())
		{
			lock_a.unlock ();
		}

		// Votes are generated for inserted or ongoing elections
		if (result.election)
		{
			node.election_helper.broadcast_vote (*result.election);
		}
		trim ();
	}
	return result;
}

// Validate a vote and apply it to the current election if one exists
nano::vote_code nano::active_transactions::vote (std::shared_ptr<nano::vote> const & vote_a)
{
	nano::vote_code result{ nano::vote_code::indeterminate };
	// If all hashes were recently confirmed then it is a replay
	unsigned recently_confirmed_counter (0);

	std::vector<std::pair<std::shared_ptr<nano::election>, nano::block_hash>> process;
	std::vector<nano::block_hash> inactive; // Hashes that should be added to inactive vote cache

	{
		auto guard{ lock () };
		for (auto const & hash : vote_a->hashes ())
		{
			auto existing (blocks.find (hash));
			if (existing != blocks.end ())
			{
				process.emplace_back (existing->second, hash);
			}
			else if (!recently_confirmed.exists (hash))
			{
				inactive.emplace_back (hash);
			}
			else
			{
				++recently_confirmed_counter;
			}
		}
	}

	// Process inactive votes outside of the critical section
	for (auto & hash : inactive)
	{
		add_inactive_vote_cache (hash, vote_a);
	}

	if (!process.empty ())
	{
		bool replay (false);
		bool processed (false);
		for (auto const & [election, block_hash] : process)
		{
			auto const result_l = node.election_helper.vote (*election, vote_a->account (), vote_a->timestamp (), block_hash);
			processed = processed || result_l.processed;
			replay = replay || result_l.replay;
		}

		// Republish vote if it is new and the node does not host a principal representative (or close to)
		if (processed)
		{
			auto const reps (node.wallets.reps ());
			if (!reps.have_half_rep () && !reps.exists (vote_a->account ()))
			{
				node.network->flood_vote (vote_a, 0.5f);
			}
		}
		result = replay ? nano::vote_code::replay : nano::vote_code::vote;
	}
	else if (recently_confirmed_counter == vote_a->hashes ().size ())
	{
		result = nano::vote_code::replay;
	}
	return result;
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
	auto exists = rsnano::rsn_active_transactions_lock_roots_exists (guard.handle, root.root ().bytes.data (), root.previous ().bytes.data ());
	return exists && blocks.find (hash) != blocks.end ();
}

bool nano::active_transactions::active (const nano::block_hash & hash) const
{
	auto guard{ lock () };
	return blocks.find (hash) != blocks.end ();
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
	auto existing = blocks.find (hash_a);
	if (existing != blocks.end ())
	{
		auto election = existing->second;
		guard.unlock ();
		result = election->winner ();
	}
	return result;
}

void nano::active_transactions::erase (nano::block const & block_a)
{
	erase (block_a.qualified_root ());
}

void nano::active_transactions::erase (nano::qualified_root const & root_a)
{
	auto guard{ lock () };
	auto election_handle = rsnano::rsn_active_transactions_lock_roots_find (guard.handle, root_a.root ().bytes.data (), root_a.previous ().bytes.data ());
	if (election_handle != nullptr)
	{
		auto election = std::make_shared<nano::election> (election_handle);
		cleanup_election (guard, election);
	}
}

void nano::active_transactions::erase_hash (nano::block_hash const & hash_a)
{
	auto guard{ lock () };
	[[maybe_unused]] auto erased (blocks.erase (hash_a));
	debug_assert (erased == 1);
}

void nano::active_transactions::erase_oldest ()
{
	auto guard{ lock () };
	if (rsnano::rsn_active_transactions_lock_roots_size (guard.handle) > 0)
	{
		std::shared_ptr<nano::election> front = list_active_impl (1, guard).front ();
		cleanup_election (guard, front);
	}
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
		result = node.election_helper.publish (block_a, *election);
		if (!result)
		{
			guard.lock ();
			blocks.emplace (block_a->hash (), election);
			guard.unlock ();
			if (auto const cache = node.inactive_vote_cache.find (block_a->hash ()); cache)
			{
				election->fill_from_cache (node.election_helper, *cache);
			}
			node.stats->inc (nano::stat::type::active, nano::stat::detail::election_block_conflict);
		}
	}
	return result;
}

// Returns the type of election status requiring callbacks calling later
boost::optional<nano::election_status_type> nano::active_transactions::confirm_block (store::transaction const & transaction_a, std::shared_ptr<nano::block> const & block_a)
{
	auto const hash = block_a->hash ();
	std::shared_ptr<nano::election> election = nullptr;
	{
		auto guard{ lock () };
		auto existing = blocks.find (hash);
		if (existing != blocks.end ())
		{
			election = existing->second;
		}
	}

	boost::optional<nano::election_status_type> status_type;
	if (election)
	{
		status_type = election->try_confirm (hash, node.election_helper);
	}
	else
	{
		status_type = nano::election_status_type::inactive_confirmation_height;
	}

	return status_type;
}

void nano::active_transactions::add_inactive_vote_cache (nano::block_hash const & hash, std::shared_ptr<nano::vote> const vote)
{
	auto rep_weight = node.ledger.weight (vote->account ());
	if (rep_weight > node.minimum_principal_weight ())
	{
		node.inactive_vote_cache.vote (hash, vote, rep_weight);
	}
}

std::size_t nano::active_transactions::election_winner_details_size ()
{
	nano::lock_guard<nano::mutex> guard{ election_winner_details_mutex };
	return election_winner_details.size ();
}

void nano::active_transactions::clear ()
{
	{
		auto guard{ lock () };
		blocks.clear ();
		rsnano::rsn_active_transactions_lock_roots_clear (guard.handle);
	}
	vacancy_update ();
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (active_transactions & active_transactions, std::string const & name)
{
	auto guard{ active_transactions.lock () };

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "roots", rsnano::rsn_active_transactions_lock_roots_size (guard.handle), sizeof (intptr_t) }));

	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocks", active_transactions.blocks.size (), sizeof (decltype (active_transactions.blocks)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "election_winner_details", active_transactions.election_winner_details_size (), sizeof (decltype (active_transactions.election_winner_details)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "normal", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::normal))), 0 }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "hinted", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::hinted))), 0 }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "optimistic", static_cast<std::size_t> (rsnano::rsn_active_transactions_lock_count_by_behavior (guard.handle, static_cast<uint8_t> (nano::election_behavior::optimistic))), 0 }));

	composite->add_component (active_transactions.recently_confirmed.collect_container_info ("recently_confirmed"));
	composite->add_component (active_transactions.recently_cemented.collect_container_info ("recently_cemented"));

	return composite;
}

/*
 * class recently_confirmed
 */

nano::recently_confirmed_cache::recently_confirmed_cache (std::size_t max_size_a) :
	max_size{ max_size_a }
{
}

void nano::recently_confirmed_cache::put (const nano::qualified_root & root, const nano::block_hash & hash)
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	confirmed.get<tag_sequence> ().emplace_back (root, hash);
	if (confirmed.size () > max_size)
	{
		confirmed.get<tag_sequence> ().pop_front ();
	}
}

void nano::recently_confirmed_cache::erase (const nano::block_hash & hash)
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	confirmed.get<tag_hash> ().erase (hash);
}

void nano::recently_confirmed_cache::clear ()
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	confirmed.clear ();
}

bool nano::recently_confirmed_cache::exists (const nano::block_hash & hash) const
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return confirmed.get<tag_hash> ().find (hash) != confirmed.get<tag_hash> ().end ();
}

bool nano::recently_confirmed_cache::exists (const nano::qualified_root & root) const
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return confirmed.get<tag_root> ().find (root) != confirmed.get<tag_root> ().end ();
}

std::size_t nano::recently_confirmed_cache::size () const
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return confirmed.size ();
}

nano::recently_confirmed_cache::entry_t nano::recently_confirmed_cache::back () const
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return confirmed.back ();
}

std::unique_ptr<nano::container_info_component> nano::recently_confirmed_cache::collect_container_info (const std::string & name)
{
	nano::unique_lock<nano::mutex> lock{ mutex };

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "confirmed", confirmed.size (), sizeof (decltype (confirmed)::value_type) }));
	return composite;
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
