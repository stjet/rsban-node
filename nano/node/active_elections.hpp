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
#include <unordered_map>

namespace nano
{
class node;
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
class active_elections_config final
{
public:
	active_elections_config () = default;
	active_elections_config (rsnano::ActiveElectionsConfigDto const & dto);
	nano::error deserialize (nano::tomlconfig & toml);
	rsnano::ActiveElectionsConfigDto into_dto () const;

public:
	// Maximum number of simultaneous active elections (AEC size)
	std::size_t size{ 5000 };
	// Limit of hinted elections as percentage of `active_elections_size`
	std::size_t hinted_limit_percentage{ 20 };
	// Limit of optimistic elections as percentage of `active_elections_size`
	std::size_t optimistic_limit_percentage{ 10 };
	// Maximum confirmation history size
	std::size_t confirmation_history_size{ 2048 };
	// Maximum cache size for recently_confirmed
	std::size_t confirmation_cache{ 65536 };
};

/**
 * Core class for determining consensus
 * Holds all active blocks i.e. recently added blocks that need confirmation
 */
class active_elections final
{
public:
	active_elections (nano::node &, rsnano::ActiveTransactionsHandle * handle);
	active_elections (active_elections const &) = delete;
	~active_elections ();

	void stop ();

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
	// Returns a list of elections sorted by difficulty
	std::vector<std::shared_ptr<nano::election>> list_active (std::size_t = std::numeric_limits<std::size_t>::max ());
	bool erase (nano::block const &);
	bool erase (nano::qualified_root const &);
	bool empty () const;
	std::size_t size () const;
	bool publish (std::shared_ptr<nano::block> const &);

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

public: // Events
	void process_confirmed (nano::election_status const & status_a, uint64_t iteration_a = 0);
	nano::tally_t tally_impl (nano::election_lock & lock) const;
	void force_confirm (nano::election & election);
	// Returns true when the winning block is durably confirmed in the ledger.
	// Later once the confirmation height processor has updated the confirmation height it will be confirmed on disk
	// It is possible for an election to be confirmed on disk but not in memory, for instance if implicitly confirmed via confirmation height
	bool confirmed (nano::election const & election) const;
	bool confirmed_locked (nano::election_lock & lock) const;
	std::vector<nano::vote_with_weight_info> votes_with_weight (nano::election & election) const;
	nano::election_extended_status current_status (nano::election & election) const;
	nano::tally_t tally (nano::election & election) const;
	void clear_recently_confirmed ();
	std::size_t recently_confirmed_size ();
	std::size_t recently_cemented_size ();
	nano::qualified_root lastest_recently_confirmed_root ();
	void insert_recently_confirmed (std::shared_ptr<nano::block> const & block);
	void insert_recently_cemented (nano::election_status const & status);
	std::deque<nano::election_status> recently_cemented_list ();

private: // Dependencies
	nano::node & node;

public:
	rsnano::ActiveTransactionsHandle * handle;

private:
	friend class election;

public: // Tests
	void clear ();

	friend class node_fork_storm_Test;
	friend class system_block_sequence_Test;
	friend class node_mass_block_new_Test;
	friend class active_elections_vote_replays_Test;
	friend class frontiers_confirmation_prioritize_frontiers_Test;
	friend class frontiers_confirmation_prioritize_frontiers_max_optimistic_elections_Test;
	friend class confirmation_height_prioritize_frontiers_overwrite_Test;
	friend class active_elections_confirmation_consistency_Test;
	friend class node_deferred_dependent_elections_Test;
	friend class active_elections_pessimistic_elections_Test;
	friend class frontiers_confirmation_expired_optimistic_elections_removal_Test;
};

}
