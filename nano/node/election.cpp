#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/node/confirmation_solicitor.hpp>
#include <nano/node/election.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>

#include <boost/format.hpp>

using namespace std::chrono;

std::chrono::milliseconds nano::election::base_latency () const
{
	return node.network_params.network.is_dev_network () ? 25ms : 1000ms;
}

nano::election_vote_result::election_vote_result (bool replay_a, bool processed_a)
{
	replay = replay_a;
	processed = processed_a;
}

nano::election_lock::election_lock (nano::election const & election) :
	handle{ rsnano::rsn_election_lock (election.handle) },
	election{ *const_cast<nano::election *> (&election) } //hack!
{
}

nano::election_lock::~election_lock ()
{
	rsnano::rsn_election_lock_destroy (handle);
}

nano::election_status nano::election_lock::status () const
{
	return { rsnano::rsn_election_lock_status (handle) };
}

void nano::election_lock::set_status (nano::election_status status)
{
	rsnano::rsn_election_lock_status_set (handle, status.handle);
}

void nano::election_lock::insert_or_assign_last_block (std::shared_ptr<nano::block> const & block)
{
	rsnano::rsn_election_lock_add_block (handle, block->get_handle ());
}

void nano::election_lock::erase_last_block (nano::block_hash const & hash)
{
	rsnano::rsn_election_lock_erase_block (handle, hash.bytes.data ());
}

size_t nano::election_lock::last_blocks_size () const
{
	return rsnano::rsn_election_lock_blocks_size (handle);
}

std::unordered_map<nano::block_hash, std::shared_ptr<nano::block>> nano::election_lock::last_blocks () const
{
	rsnano::BlockArrayDto dto;
	rsnano::rsn_election_lock_blocks (handle, &dto);
	std::vector<std::shared_ptr<nano::block>> blocks;
	rsnano::read_block_array_dto (dto, blocks);

	std::unordered_map<nano::block_hash, std::shared_ptr<nano::block>> result;
	for (auto block : blocks)
	{
		result.insert ({ block->hash (), block });
	}
	return result;
}

std::shared_ptr<nano::block> nano::election_lock::find_block (nano::block_hash const & hash)
{
	auto block_handle = rsnano::rsn_election_lock_blocks_find (handle, hash.bytes.data ());
	std::shared_ptr<nano::block> result{};
	if (block_handle != nullptr)
	{
		result = nano::block_handle_to_block (block_handle);
	}
	return result;
}

void nano::election_lock::insert_or_assign_vote (nano::account const & account, nano::vote_info const & vote_info)
{
	rsnano::rsn_election_lock_votes_insert (handle, account.bytes.data (), vote_info.handle);
}

std::optional<nano::vote_info> nano::election_lock::find_vote (nano::account const & account) const
{
	auto existing{ rsnano::rsn_election_lock_votes_find (handle, account.bytes.data ()) };
	if (existing != nullptr)
	{
		return nano::vote_info{ existing };
	}
	else
	{
		return {};
	}
}

size_t nano::election_lock::last_votes_size () const
{
	return rsnano::rsn_election_lock_votes_size (handle);
}

std::unordered_map<nano::account, nano::vote_info> nano::election_lock::last_votes () const
{
	auto result_handle = rsnano::rsn_election_lock_votes (handle);
	std::unordered_map<nano::account, nano::vote_info> result;
	auto len = rsnano::rsn_vote_info_collection_len (result_handle);
	for (auto i = 0; i < len; ++i)
	{
		nano::account account;
		auto info_handle = rsnano::rsn_vote_info_collection_get (result_handle, i, account.bytes.data ());
		result.insert_or_assign (account, nano::vote_info{ info_handle });
	}
	rsnano::rsn_vote_info_collection_destroy (result_handle);

	return result;
}

void nano::election_lock::erase_vote (nano::account const & account)
{
	rsnano::rsn_election_lock_votes_erase (handle, account.bytes.data ());
}

void nano::election_lock::unlock ()
{
	rsnano::rsn_election_lock_unlock (handle);
}

void nano::election_lock::lock ()
{
	rsnano::rsn_election_lock_lock (handle, election.handle);
}

nano::election::election (nano::node & node_a, std::shared_ptr<nano::block> const & block_a, std::function<void (std::shared_ptr<nano::block> const &)> const & confirmation_action_a, std::function<void (nano::account const &)> const & live_vote_action_a, nano::election_behavior election_behavior_a) :
	confirmation_action (confirmation_action_a),
	live_vote_action (live_vote_action_a),
	node (node_a),
	behavior_m (election_behavior_a),
	handle{ rsnano::rsn_election_create (block_a->get_handle ()) }
{
	nano::election_status status;
	status.set_winner (block_a);
	status.set_election_end (std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::system_clock::now ().time_since_epoch ()));
	status.set_block_count (1);
	status.set_election_status_type (nano::election_status_type::ongoing);
	{
		auto guard{ lock () };
		guard.set_status (status);
		guard.insert_or_assign_vote (nano::account::null (), nano::vote_info{ 0, block_a->hash () });
		guard.insert_or_assign_last_block (block_a);
	}
}

nano::election::~election ()
{
	rsnano::rsn_election_destroy (handle);
}

nano::qualified_root nano::election::qualified_root () const
{
	nano::qualified_root result;
	rsnano::rsn_election_qualified_root (handle, result.uint256s[0].bytes.data (), result.uint256s[1].bytes.data ());
	return result;
}

nano::root nano::election::root () const
{
	nano::root root;
	rsnano::rsn_election_root (handle, root.bytes.data ());
	return root;
}

void nano::election::confirm_once (nano::election_lock & lock_a, nano::election_status_type type_a)
{
	// This must be kept above the setting of election state, as dependent confirmed elections require up to date changes to election_winner_details
	nano::unique_lock<nano::mutex> election_winners_lk{ node.active.election_winner_details_mutex };
	auto status_l{ lock_a.status () };
	if (state_m.exchange (nano::election::state_t::confirmed) != nano::election::state_t::confirmed && (node.active.election_winner_details.count (status_l.get_winner ()->hash ()) == 0))
	{
		node.active.election_winner_details.emplace (status_l.get_winner ()->hash (), shared_from_this ());
		election_winners_lk.unlock ();
		status_l.set_election_end (std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::system_clock::now ().time_since_epoch ()));
		status_l.set_election_duration (std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::steady_clock::now () - election_start));
		status_l.set_confirmation_request_count (confirmation_request_count);
		status_l.set_block_count (nano::narrow_cast<decltype (status_l.get_block_count ())> (lock_a.last_blocks_size ()));
		status_l.set_voter_count (nano::narrow_cast<decltype (status_l.get_voter_count ())> (lock_a.last_votes_size ()));
		status_l.set_election_status_type (type_a);
		lock_a.set_status (status_l);
		lock_a.unlock ();

		node.background ([node_l = node.shared (), status_l, confirmation_action_l = confirmation_action] () {
			node_l->process_confirmed (status_l);

			if (confirmation_action_l)
			{
				confirmation_action_l (status_l.get_winner ());
			}
		});
	}
	else
	{
		lock_a.unlock ();
	}
}

bool nano::election::valid_change (nano::election::state_t expected_a, nano::election::state_t desired_a) const
{
	bool result = false;
	switch (expected_a)
	{
		case nano::election::state_t::passive:
			switch (desired_a)
			{
				case nano::election::state_t::active:
				case nano::election::state_t::confirmed:
				case nano::election::state_t::expired_unconfirmed:
					result = true;
					break;
				default:
					break;
			}
			break;
		case nano::election::state_t::active:
			switch (desired_a)
			{
				case nano::election::state_t::confirmed:
				case nano::election::state_t::expired_unconfirmed:
					result = true;
					break;
				default:
					break;
			}
			break;
		case nano::election::state_t::confirmed:
			switch (desired_a)
			{
				case nano::election::state_t::expired_confirmed:
					result = true;
					break;
				default:
					break;
			}
			break;
		case nano::election::state_t::expired_unconfirmed:
		case nano::election::state_t::expired_confirmed:
			break;
	}
	return result;
}

bool nano::election::state_change (nano::election::state_t expected_a, nano::election::state_t desired_a)
{
	bool result = true;
	if (valid_change (expected_a, desired_a))
	{
		if (state_m.compare_exchange_strong (expected_a, desired_a))
		{
			state_start = std::chrono::steady_clock::now ().time_since_epoch ();
			result = false;
		}
	}
	return result;
}

bool nano::election::confirmed (nano::election_lock & lock) const
{
	return node.block_confirmed (lock.status ().get_winner ()->hash ());
}

std::chrono::milliseconds nano::election::confirm_req_time () const
{
	switch (behavior ())
	{
		case election_behavior::normal:
		case election_behavior::hinted:
			return base_latency () * 5;
		case election_behavior::optimistic:
			return base_latency () * 2;
	}
	debug_assert (false);
	return {};
}

void nano::election::send_confirm_req (nano::confirmation_solicitor & solicitor_a)
{
	if (confirm_req_time () < (std::chrono::steady_clock::now () - last_req))
	{
		auto guard{ lock () };
		if (!solicitor_a.add (*this, guard))
		{
			last_req = std::chrono::steady_clock::now ();
			++confirmation_request_count;
		}
	}
}

void nano::election::transition_active ()
{
	state_change (nano::election::state_t::passive, nano::election::state_t::active);
}

bool nano::election::confirmed () const
{
	auto guard{ lock () };
	return confirmed (guard);
}

bool nano::election::status_confirmed () const
{
	return state_m == nano::election::state_t::confirmed || state_m == nano::election::state_t::expired_confirmed;
}

bool nano::election::failed () const
{
	return state_m == nano::election::state_t::expired_unconfirmed;
}

void nano::election::broadcast_block (nano::confirmation_solicitor & solicitor_a)
{
	if (base_latency () * 15 < std::chrono::steady_clock::now () - last_block)
	{
		auto guard{ lock () };
		if (!solicitor_a.broadcast (*this, guard))
		{
			last_block = std::chrono::steady_clock::now ();
		}
	}
}

void nano::election::broadcast_vote ()
{
	nano::election_lock guard{ *this };
	if (last_vote + std::chrono::milliseconds (node.config->network_params.network.vote_broadcast_interval) < std::chrono::steady_clock::now ())
	{
		broadcast_vote_impl (guard);
		last_vote = std::chrono::steady_clock::now ();
	}
}

nano::vote_info nano::election::get_last_vote (nano::account const & account)
{
	auto guard{ lock () };
	return *guard.find_vote (account);
}

void nano::election::set_last_vote (nano::account const & account, nano::vote_info vote_info)
{
	auto guard{ lock () };
	guard.insert_or_assign_vote (account, vote_info);
}

nano::election_status nano::election::get_status () const
{
	auto guard{ lock () };
	return guard.status ();
}

nano::election_lock nano::election::lock () const
{
	return nano::election_lock{ *this };
}

bool nano::election::transition_time (nano::confirmation_solicitor & solicitor_a)
{
	bool result = false;
	switch (state_m)
	{
		case nano::election::state_t::passive:
			if (base_latency () * passive_duration_factor < std::chrono::steady_clock::now ().time_since_epoch () - state_start.load ())
			{
				state_change (nano::election::state_t::passive, nano::election::state_t::active);
			}
			break;
		case nano::election::state_t::active:
			broadcast_vote ();
			broadcast_block (solicitor_a);
			send_confirm_req (solicitor_a);
			break;
		case nano::election::state_t::confirmed:
			result = true; // Return true to indicate this election should be cleaned up
			state_change (nano::election::state_t::confirmed, nano::election::state_t::expired_confirmed);
			break;
		case nano::election::state_t::expired_unconfirmed:
		case nano::election::state_t::expired_confirmed:
			debug_assert (false);
			break;
	}

	if (!confirmed () && time_to_live () < std::chrono::steady_clock::now () - election_start)
	{
		auto guard{ lock () };
		// It is possible the election confirmed while acquiring the mutex
		// state_change returning true would indicate it
		if (!state_change (state_m.load (), nano::election::state_t::expired_unconfirmed))
		{
			result = true; // Return true to indicate this election should be cleaned up
			if (node.config->logging.election_expiration_tally_logging ())
			{
				log_votes (guard, tally_impl (guard), "Election expired: ");
			}
			auto st{ guard.status () };
			st.set_election_status_type (nano::election_status_type::stopped);
			guard.set_status (st);
		}
	}
	return result;
}

std::chrono::milliseconds nano::election::time_to_live () const
{
	switch (behavior ())
	{
		case election_behavior::normal:
			return std::chrono::milliseconds (5 * 60 * 1000);
		case election_behavior::hinted:
		case election_behavior::optimistic:
			return std::chrono::milliseconds (30 * 1000);
	}
	debug_assert (false);
	return {};
}

std::chrono::seconds nano::election::cooldown_time (nano::uint128_t weight) const
{
	auto online_stake = node.online_reps.trended ();
	if (weight > online_stake / 20) // Reps with more than 5% weight
	{
		return std::chrono::seconds{ 1 };
	}
	if (weight > online_stake / 100) // Reps with more than 1% weight
	{
		return std::chrono::seconds{ 5 };
	}
	// The rest of smaller reps
	return std::chrono::seconds{ 15 };
}

bool nano::election::have_quorum (nano::tally_t const & tally_a) const
{
	auto i (tally_a.begin ());
	++i;
	auto second (i != tally_a.end () ? i->first : 0);
	auto delta_l (node.online_reps.delta ());
	release_assert (tally_a.begin ()->first >= second);
	bool result{ (tally_a.begin ()->first - second) >= delta_l };
	return result;
}

nano::tally_t nano::election::tally () const
{
	auto guard{ lock () };
	return tally_impl (guard);
}

nano::tally_t nano::election::tally_impl (nano::election_lock & lock) const
{
	std::unordered_map<nano::block_hash, nano::uint128_t> block_weights;
	std::unordered_map<nano::block_hash, nano::uint128_t> final_weights_l;
	for (auto const & [account, info] : lock.last_votes ())
	{
		auto rep_weight (node.ledger.weight (account));
		block_weights[info.get_hash ()] += rep_weight;
		if (info.get_timestamp () == std::numeric_limits<uint64_t>::max ())
		{
			final_weights_l[info.get_hash ()] += rep_weight;
		}
	}
	last_tally = block_weights;
	nano::tally_t result;
	for (auto const & [hash, amount] : block_weights)
	{
		auto block (lock.find_block (hash));
		if (block != nullptr)
		{
			result.emplace (amount, block);
		}
	}
	// Calculate final votes sum for winner
	if (!final_weights_l.empty () && !result.empty ())
	{
		auto winner_hash (result.begin ()->second->hash ());
		auto find_final (final_weights_l.find (winner_hash));
		if (find_final != final_weights_l.end ())
		{
			final_weight = find_final->second;
		}
	}
	return result;
}

void nano::election::confirm_if_quorum (nano::election_lock & lock_a)
{
	auto tally_l (tally_impl (lock_a));
	debug_assert (!tally_l.empty ());
	auto winner (tally_l.begin ());
	auto block_l (winner->second);
	auto winner_hash_l{ block_l->hash () };
	auto status_l{ lock_a.status () };
	status_l.set_tally (winner->first);
	status_l.set_final_tally (final_weight);
	auto status_winner_hash_l{ status_l.get_winner ()->hash () };
	nano::uint128_t sum (0);
	for (auto & i : tally_l)
	{
		sum += i.first;
	}
	if (sum >= node.online_reps.delta () && winner_hash_l != status_winner_hash_l)
	{
		status_l.set_winner (block_l);
		remove_votes (lock_a, status_winner_hash_l);
		node.block_processor.force (block_l);
	}

	lock_a.set_status (status_l);

	if (have_quorum (tally_l))
	{
		if (node.ledger.cache.final_votes_confirmation_canary () && !is_quorum.exchange (true) && node.config->enable_voting && node.wallets.reps ().voting > 0)
		{
			auto hash = status_l.get_winner ()->hash ();
			lock_a.unlock ();
			node.final_generator.add (root (), hash);
			lock_a.lock ();
		}
		if (!node.ledger.cache.final_votes_confirmation_canary () || final_weight >= node.online_reps.delta ())
		{
			if (node.config->logging.vote_logging () || (node.config->logging.election_fork_tally_logging () && lock_a.last_blocks_size () > 1))
			{
				log_votes (lock_a, tally_l);
			}
			confirm_once (lock_a, nano::election_status_type::active_confirmed_quorum);
		}
	}
}

boost::optional<nano::election_status_type> nano::election::try_confirm (nano::block_hash const & hash)
{
	boost::optional<nano::election_status_type> status_type;
	auto guard{ lock () };
	auto winner = guard.status ().get_winner ();
	if (winner && winner->hash () == hash)
	{
		// Determine if the block was confirmed explicitly via election confirmation or implicitly via confirmation height
		if (!status_confirmed ())
		{
			confirm_once (guard, nano::election_status_type::active_confirmation_height);
			status_type = nano::election_status_type::active_confirmation_height;
		}
		else
		{
			status_type = nano::election_status_type::active_confirmed_quorum;
		}
	}
	else
	{
		status_type = boost::optional<nano::election_status_type>{};
	}
	return status_type;
}

void nano::election::set_status_type (nano::election_status_type status_type)
{
	nano::election_lock election_lk{ *this };
	auto st{ election_lk.status () };
	st.set_election_status_type (status_type);
	st.set_confirmation_request_count (confirmation_request_count);
	election_lk.set_status (st);
}

void nano::election::log_votes (nano::election_lock & lock, nano::tally_t const & tally_a, std::string const & prefix_a) const
{
	std::stringstream tally;
	std::string line_end (node.config->logging.single_line_record () ? "\t" : "\n");
	tally << boost::str (boost::format ("%1%%2%Vote tally for root %3%, final weight:%4%") % prefix_a % line_end % root ().to_string () % final_weight);
	for (auto i (tally_a.begin ()), n (tally_a.end ()); i != n; ++i)
	{
		tally << boost::str (boost::format ("%1%Block %2% weight %3%") % line_end % i->second->hash ().to_string () % i->first.convert_to<std::string> ());
	}
	auto votes{ lock.last_votes () };
	for (auto i (votes.begin ()), n (votes.end ()); i != n; ++i)
	{
		if (i->first != nullptr)
		{
			tally << boost::str (boost::format ("%1%%2% %3% %4%") % line_end % i->first.to_account () % std::to_string (i->second.get_timestamp ()) % i->second.get_hash ().to_string ());
		}
	}
	node.logger->try_log (tally.str ());
}

std::shared_ptr<nano::block> nano::election::find (nano::block_hash const & hash_a) const
{
	nano::election_lock guard{ *this };
	return guard.find_block (hash_a);
}

nano::election_vote_result nano::election::vote (nano::account const & rep, uint64_t timestamp_a, nano::block_hash const & block_hash_a, vote_source vote_source_a)
{
	auto weight = node.ledger.weight (rep);
	if (!node.network_params.network.is_dev_network () && weight <= node.minimum_principal_weight ())
	{
		return nano::election_vote_result (false, false);
	}
	nano::election_lock lock{ *this };

	auto last_vote_l{ lock.find_vote (rep) };
	if (last_vote_l.has_value ())
	{
		if (last_vote_l->get_timestamp () > timestamp_a)
		{
			return nano::election_vote_result (true, false);
		}
		if (last_vote_l->get_timestamp () == timestamp_a && !(last_vote_l->get_hash () < block_hash_a))
		{
			return nano::election_vote_result (true, false);
		}

		auto max_vote = timestamp_a == std::numeric_limits<uint64_t>::max () && last_vote_l->get_timestamp () < timestamp_a;

		bool past_cooldown = true;
		// Only cooldown live votes
		if (vote_source_a == vote_source::live)
		{
			const auto cooldown = cooldown_time (weight);
			past_cooldown = last_vote_l->get_time () <= std::chrono::system_clock::now () - cooldown;
		}

		if (!max_vote && !past_cooldown)
		{
			return nano::election_vote_result (false, false);
		}
	}
	lock.insert_or_assign_vote (rep, { timestamp_a, block_hash_a });
	if (vote_source_a == vote_source::live)
	{
		live_vote_action (rep);
	}

	node.stats->inc (nano::stat::type::election, vote_source_a == vote_source::live ? nano::stat::detail::vote_new : nano::stat::detail::vote_cached);

	if (!confirmed (lock))
	{
		confirm_if_quorum (lock);
	}
	return nano::election_vote_result (false, true);
}

std::size_t nano::election::fill_from_cache (nano::vote_cache::entry const & entry)
{
	std::size_t inserted = 0;
	for (const auto & [rep, timestamp] : entry.voters)
	{
		auto [is_replay, processed] = vote (rep, timestamp, entry.hash, nano::election::vote_source::cache);
		if (processed)
		{
			inserted++;
		}
	}
	return inserted;
}

bool nano::election::publish (std::shared_ptr<nano::block> const & block_a)
{
	nano::election_lock lock{ *this };

	// Do not insert new blocks if already confirmed
	auto result = confirmed (lock);
	if (!result && lock.last_blocks_size () >= max_blocks && lock.find_block (block_a->hash ()) == nullptr)
	{
		if (!replace_by_weight (lock, block_a->hash ()))
		{
			result = true;
			node.network->tcp_channels->publish_filter->clear (block_a);
		}
	}
	if (!result)
	{
		auto existing = lock.find_block (block_a->hash ());
		if (existing == nullptr)
		{
			lock.insert_or_assign_last_block (block_a);
		}
		else
		{
			result = true;
			lock.insert_or_assign_last_block (block_a);
			auto st{ lock.status () };
			if (st.get_winner ()->hash () == block_a->hash ())
			{
				st.set_winner (block_a);
				lock.set_status (st);
				node.network->flood_block (block_a, nano::transport::buffer_drop_policy::no_limiter_drop);
			}
		}
	}
	/*
	Result is true if:
	1) election is confirmed or expired
	2) given election contains 10 blocks & new block didn't receive enough votes to replace existing blocks
	3) given block in already in election & election contains less than 10 blocks (replacing block content with new)
	*/
	return result;
}

nano::election_extended_status nano::election::current_status () const
{
	nano::election_lock guard{ *this };
	nano::election_status status_l = guard.status ();
	status_l.set_confirmation_request_count (confirmation_request_count);
	status_l.set_block_count (nano::narrow_cast<decltype (status_l.get_block_count ())> (guard.last_blocks_size ()));
	status_l.set_voter_count (nano::narrow_cast<decltype (status_l.get_voter_count ())> (guard.last_votes_size ()));
	return nano::election_extended_status{ status_l, guard.last_votes (), tally_impl (guard) };
}

std::shared_ptr<nano::block> nano::election::winner () const
{
	nano::election_lock guard{ *this };
	return guard.status ().get_winner ();
}

void nano::election::broadcast_vote_impl (nano::election_lock & lock)
{
	if (node.config->enable_voting && node.wallets.reps ().voting > 0)
	{
		node.stats->inc (nano::stat::type::election, nano::stat::detail::generate_vote);

		if (confirmed (lock) || have_quorum (tally_impl (lock)))
		{
			node.stats->inc (nano::stat::type::election, nano::stat::detail::generate_vote_final);
			node.final_generator.add (root (), lock.status ().get_winner ()->hash ()); // Broadcasts vote to the network
		}
		else
		{
			node.stats->inc (nano::stat::type::election, nano::stat::detail::generate_vote_normal);
			node.generator.add (root (), lock.status ().get_winner ()->hash ()); // Broadcasts vote to the network
		}
	}
}

void nano::election::remove_votes (nano::election_lock & lock, nano::block_hash const & hash_a)
{
	if (node.config->enable_voting && node.wallets.reps ().voting > 0)
	{
		// Remove votes from election
		auto list_generated_votes (node.history.votes (root (), hash_a));
		for (auto const & vote : list_generated_votes)
		{
			lock.erase_vote (vote->account ());
		}
		// Clear votes cache
		node.history.erase (root ());
	}
}

void nano::election::remove_block (nano::election_lock & lock, nano::block_hash const & hash_a)
{
	if (lock.status ().get_winner ()->hash () != hash_a)
	{
		if (auto existing = lock.find_block (hash_a); existing != nullptr)
		{
			auto votes{ lock.last_votes () };
			for (auto & i : votes)
			{
				if (i.second.get_hash () == hash_a)
				{
					lock.erase_vote (i.first);
				}
			}
			node.network->tcp_channels->publish_filter->clear (existing);
			lock.erase_last_block (hash_a);
		}
	}
}

bool nano::election::replace_by_weight (nano::election_lock & lock_a, nano::block_hash const & hash_a)
{
	nano::block_hash replaced_block (0);
	auto winner_hash (lock_a.status ().get_winner ()->hash ());
	// Sort existing blocks tally
	std::vector<std::pair<nano::block_hash, nano::uint128_t>> sorted;
	sorted.reserve (last_tally.size ());
	std::copy (last_tally.begin (), last_tally.end (), std::back_inserter (sorted));
	lock_a.unlock ();
	// Sort in ascending order
	std::sort (sorted.begin (), sorted.end (), [] (auto const & left, auto const & right) { return left.second < right.second; });
	// Replace if lowest tally is below inactive cache new block weight
	auto inactive_existing = node.inactive_vote_cache.find (hash_a);
	auto inactive_tally = inactive_existing ? inactive_existing->tally : 0;
	if (inactive_tally > 0 && sorted.size () < max_blocks)
	{
		// If count of tally items is less than 10, remove any block without tally
		for (auto const & [hash, block] : blocks ())
		{
			if (std::find_if (sorted.begin (), sorted.end (), [&hash = hash] (auto const & item_a) { return item_a.first == hash; }) == sorted.end () && hash != winner_hash)
			{
				replaced_block = hash;
				break;
			}
		}
	}
	else if (inactive_tally > 0 && inactive_tally > sorted.front ().second)
	{
		if (sorted.front ().first != winner_hash)
		{
			replaced_block = sorted.front ().first;
		}
		else if (inactive_tally > sorted[1].second)
		{
			// Avoid removing winner
			replaced_block = sorted[1].first;
		}
	}

	bool replaced (false);
	if (!replaced_block.is_zero ())
	{
		node.active.erase_hash (replaced_block);
		lock_a.lock ();
		remove_block (lock_a, replaced_block);
		replaced = true;
	}
	else
	{
		lock_a.lock ();
	}
	return replaced;
}

void nano::election::force_confirm (nano::election_status_type type_a)
{
	release_assert (node.network_params.network.is_dev_network ());
	nano::election_lock lock{ *this };
	confirm_once (lock, type_a);
}

std::unordered_map<nano::block_hash, std::shared_ptr<nano::block>> nano::election::blocks () const
{
	nano::election_lock guard{ *this };
	return guard.last_blocks ();
}

std::unordered_map<nano::account, nano::vote_info> nano::election::votes () const
{
	nano::election_lock guard{ *this };
	return guard.last_votes ();
}

std::vector<nano::vote_with_weight_info> nano::election::votes_with_weight () const
{
	std::multimap<nano::uint128_t, nano::vote_with_weight_info, std::greater<nano::uint128_t>> sorted_votes;
	std::vector<nano::vote_with_weight_info> result;
	auto votes_l (votes ());
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

nano::stat::detail nano::to_stat_detail (nano::election_behavior behavior)
{
	switch (behavior)
	{
		case nano::election_behavior::normal:
		{
			return nano::stat::detail::normal;
		}
		case nano::election_behavior::hinted:
		{
			return nano::stat::detail::hinted;
		}
		case nano::election_behavior::optimistic:
		{
			return nano::stat::detail::optimistic;
		}
	}

	debug_assert (false, "unknown election behavior");
	return {};
}

nano::election_behavior nano::election::behavior () const
{
	return behavior_m;
}
