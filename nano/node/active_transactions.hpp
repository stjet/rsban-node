#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/node/election.hpp>
#include <nano/node/election_behavior.hpp>
#include <nano/node/election_insertion_result.hpp>
#include <nano/node/election_status.hpp>
#include <nano/node/vote_with_weight_info.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/multi_index/hashed_index.hpp>
#include <boost/multi_index/member.hpp>
#include <boost/multi_index/random_access_index.hpp>
#include <boost/multi_index/sequenced_index.hpp>
#include <boost/multi_index_container.hpp>

#include <deque>
#include <memory>
#include <thread>
#include <unordered_map>

namespace nano
{
class node;
class active_transactions;
class block;
class block_sideband;
class block_processor;
class election;
class vote;
class confirming_set;
class stats;
class election_lock;
}

namespace nano::store
{
class read_transaction;
}

namespace nano
{
/**
 * Core class for determining consensus
 * Holds all active blocks i.e. recently added blocks that need confirmation
 */
class active_transactions final
{
public:
	active_transactions (nano::node &, nano::confirming_set &, nano::block_processor &);
	active_transactions (active_transactions const &) = delete;
	~active_transactions ();

	void start ();
	void stop ();

	/**
	 * Starts new election with a specified behavior type
	 */
	nano::election_insertion_result insert (std::shared_ptr<nano::block> const & block, nano::election_behavior behavior = nano::election_behavior::normal);
	// Distinguishes replay votes, cannot be determined if the block is not in any election
	std::unordered_map<nano::block_hash, nano::vote_code> vote (std::shared_ptr<nano::vote> const &, nano::vote_source = nano::vote_source::live);
	// Is the root of this block in the roots container
	bool active (nano::block const &) const;
	bool active (nano::qualified_root const &) const;
	/**
	 * Is the block hash present in any active election
	 */
	bool active (nano::block_hash const &) const;
	std::shared_ptr<nano::election> election (nano::qualified_root const &) const;
	std::shared_ptr<nano::block> winner (nano::block_hash const &) const;
	// Returns a list of elections sorted by difficulty
	std::vector<std::shared_ptr<nano::election>> list_active (std::size_t = std::numeric_limits<std::size_t>::max ());
	bool erase (nano::block const &);
	bool erase (nano::qualified_root const &);
	bool empty () const;
	std::size_t size () const;
	bool publish (std::shared_ptr<nano::block> const &);
	void block_cemented_callback (std::shared_ptr<nano::block> const &);
	void block_already_cemented_callback (nano::block_hash const &);

	/**
	 * Maximum number of elections that should be present in this container
	 * NOTE: This is only a soft limit, it is possible for this container to exceed this count
	 */
	int64_t limit (nano::election_behavior behavior = nano::election_behavior::normal) const;
	/**
	 * How many election slots are available for specified election type
	 */
	int64_t vacancy (nano::election_behavior behavior = nano::election_behavior::normal) const;
	void set_vacancy_update (std::function<void ()> callback);
	void vacancy_update ();

	std::size_t election_winner_details_size ();
	void add_election_winner_details (nano::block_hash const &, std::shared_ptr<nano::election> const &);
	std::shared_ptr<nano::election> remove_election_winner_details (nano::block_hash const &);

public: // Events
	void add_vote_processed_observer (std::function<void (std::shared_ptr<nano::vote> const &, nano::vote_source, std::unordered_map<nano::block_hash, nano::vote_code> const &)> observer);

	void process_confirmed (nano::election_status const & status_a, uint64_t iteration_a = 0);
	nano::tally_t tally_impl (nano::election_lock & lock) const;
	void remove_votes (nano::election & election, nano::election_lock & lock, nano::block_hash const & hash_a);
	void force_confirm (nano::election & election);
	// Returns true when the winning block is durably confirmed in the ledger.
	// Later once the confirmation height processor has updated the confirmation height it will be confirmed on disk
	// It is possible for an election to be confirmed on disk but not in memory, for instance if implicitly confirmed via confirmation height
	bool confirmed (nano::election const & election) const;
	bool confirmed_locked (nano::election_lock & lock) const;
	bool confirmed (nano::block_hash const & hash) const;
	std::vector<nano::vote_with_weight_info> votes_with_weight (nano::election & election) const;
	bool publish (std::shared_ptr<nano::block> const & block_a, nano::election & election);
	/*
	 * Process vote. Internally uses cooldown to throttle non-final votes
	 * If the election reaches consensus, it will be confirmed
	 */
	nano::vote_code vote (nano::election & election, nano::account const & rep, uint64_t timestamp_a, nano::block_hash const & block_hash_a, nano::vote_source vote_source_a = nano::vote_source::live);
	nano::election_extended_status current_status (nano::election & election) const;
	nano::tally_t tally (nano::election & election) const;
	void clear_recently_confirmed ();
	std::size_t recently_confirmed_size ();
	std::size_t recently_cemented_size ();
	bool recently_confirmed (nano::block_hash const & hash);
	nano::qualified_root lastest_recently_confirmed_root ();
	void insert_recently_confirmed (std::shared_ptr<nano::block> const & block);
	void insert_recently_cemented (nano::election_status const & status);
	std::deque<nano::election_status> recently_cemented_list ();

private:
	void request_loop ();

private: // Dependencies
	nano::node & node;
	nano::block_processor & block_processor;

private:
	std::thread thread;

public:
	rsnano::ActiveTransactionsHandle * handle;

private:
	friend class election;
	friend std::unique_ptr<container_info_component> collect_container_info (active_transactions &, std::string const &);

public: // Tests
	void clear ();

	friend class node_fork_storm_Test;
	friend class system_block_sequence_Test;
	friend class node_mass_block_new_Test;
	friend class active_transactions_vote_replays_Test;
	friend class frontiers_confirmation_prioritize_frontiers_Test;
	friend class frontiers_confirmation_prioritize_frontiers_max_optimistic_elections_Test;
	friend class confirmation_height_prioritize_frontiers_overwrite_Test;
	friend class active_transactions_confirmation_consistency_Test;
	friend class node_deferred_dependent_elections_Test;
	friend class active_transactions_pessimistic_elections_Test;
	friend class frontiers_confirmation_expired_optimistic_elections_removal_Test;
};

std::unique_ptr<container_info_component> collect_container_info (active_transactions & active_transactions, std::string const & name);
}
