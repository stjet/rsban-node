#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/node/confirmation_solicitor.hpp>
#include <nano/node/election.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <iostream>

using namespace std::chrono;

nano::election_vote_result::election_vote_result (bool replay_a, bool processed_a)
{
	replay = replay_a;
	processed = processed_a;
}

/*
 * election_lock
 */

nano::election_lock::election_lock (nano::election const & election) :
	handle{ rsnano::rsn_election_lock (election.handle) },
	election{ *const_cast<nano::election *> (&election) } // hack!
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

bool nano::election_lock::state_change (nano::election_state expected_a, nano::election_state desired_a)
{
	return rsnano::rsn_election_lock_state_change (handle, static_cast<uint8_t> (expected_a), static_cast<uint8_t> (desired_a));
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

void nano::election_lock::set_final_weight (nano::amount const & weight)
{
	rsnano::rsn_election_lock_final_weight_set (handle, weight.bytes.data ());
}

nano::amount nano::election_lock::final_weight () const
{
	nano::amount weight;
	rsnano::rsn_election_lock_final_weight (handle, weight.bytes.data ());
	return weight;
}

void nano::election_lock::unlock ()
{
	rsnano::rsn_election_lock_unlock (handle);
}

void nano::election_lock::lock ()
{
	rsnano::rsn_election_lock_lock (handle, election.handle);
}

/*
 * election
 */

namespace
{
void confirmation_callback (void * context, rsnano::BlockHandle * block_handle)
{
	try
	{
		auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context);
		auto block{ nano::block_handle_to_block (block_handle) };
		if ((*callback) != nullptr)
		{
			(*callback) (block);
		}
	}
	catch (std::exception e)
	{
		std::cerr << "Exception in confirmation_callback: " << e.what () << std::endl;
	}
}

void delete_confirmation_context (void * context)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context);
	delete callback;
}

void live_vote_callback (void * context, uint8_t const * account_bytes)
{
	try
	{
		auto callback = static_cast<std::function<void (nano::account const &)> *> (context);
		auto account = nano::account::from_bytes (account_bytes);
		if ((*callback) != nullptr)
		{
			(*callback) (account);
		}
	}
	catch (std::exception e)
	{
		std::cerr << "Exception in live_vote_callback: " << e.what () << std::endl;
	}
}

void delete_live_vote_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::account const &)> *> (context);
	delete callback;
}
}

nano::election::election (nano::node & node_a, std::shared_ptr<nano::block> const & block_a, std::function<void (std::shared_ptr<nano::block> const &)> const & confirmation_action_a, std::function<void (nano::account const &)> const & live_vote_action_a, nano::election_behavior election_behavior_a) :
	handle{
		rsnano::rsn_election_create (
		block_a->get_handle (),
		static_cast<uint8_t> (election_behavior_a),
		confirmation_callback,
		new std::function<void (std::shared_ptr<nano::block> const &)> (confirmation_action_a),
		delete_confirmation_context,
		live_vote_callback,
		new std::function<void (nano::account const &)> (live_vote_action_a),
		delete_live_vote_context)
	}
{
}

nano::election::election (rsnano::ElectionHandle * handle_a) :
	handle{ handle_a }
{
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

bool nano::election::is_quorum () const
{
	return rsnano::rsn_election_is_quorum (handle);
}

void nano::election::transition_active ()
{
	auto guard{ lock () };
	guard.state_change (nano::election_state::passive, nano::election_state::active);
}

bool nano::election::failed () const
{
	auto guard{ lock () };
	auto state_l = static_cast<nano::election_state> (rsnano::rsn_election_lock_state (guard.handle));
	return state_l == nano::election_state::expired_unconfirmed;
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

unsigned nano::election::get_confirmation_request_count () const
{
	return rsnano::rsn_election_confirmation_request_count (handle);
}

void nano::election::inc_confirmation_request_count ()
{
	rsnano::rsn_election_confirmation_request_count_inc (handle);
}

void nano::election::set_status_type (nano::election_status_type status_type)
{
	nano::election_lock election_lk{ *this };
	auto st{ election_lk.status () };
	st.set_election_status_type (status_type);
	st.set_confirmation_request_count (get_confirmation_request_count ());
	election_lk.set_status (st);
}

std::shared_ptr<nano::block> nano::election::find (nano::block_hash const & hash_a) const
{
	nano::election_lock guard{ *this };
	return guard.find_block (hash_a);
}

std::shared_ptr<nano::block> nano::election::winner () const
{
	nano::election_lock guard{ *this };
	return guard.status ().get_winner ();
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

nano::stat::detail nano::to_stat_detail (nano::election_behavior behavior)
{
	auto val = rsnano::rsn_election_behaviour_into_stat_detail (static_cast<uint8_t> (behavior));
	return static_cast<nano::stat::detail> (val);
}

nano::election_behavior nano::election::behavior () const
{
	return static_cast<nano::election_behavior> (rsnano::rsn_election_behavior (handle));
}
