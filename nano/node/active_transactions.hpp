#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/node/election.hpp>
#include <nano/node/election_insertion_result.hpp>
#include <nano/node/voting.hpp>
#include <nano/secure/common.hpp>

#include <boost/multi_index/hashed_index.hpp>
#include <boost/multi_index/member.hpp>
#include <boost/multi_index/random_access_index.hpp>
#include <boost/multi_index/sequenced_index.hpp>
#include <boost/multi_index_container.hpp>

#include <atomic>
#include <condition_variable>
#include <deque>
#include <memory>
#include <unordered_map>

namespace mi = boost::multi_index;

namespace nano
{
class node;
class active_transactions;
class block;
class block_sideband;
class block_processor;
class election;
class vote;
class confirmation_height_processor;
class stats;

class recently_confirmed_cache final
{
public:
	using entry_t = std::pair<nano::qualified_root, nano::block_hash>;

	explicit recently_confirmed_cache (std::size_t max_size);

	void put (nano::qualified_root const &, nano::block_hash const &);
	void erase (nano::block_hash const &);
	void clear ();
	std::size_t size () const;

	bool exists (nano::qualified_root const &) const;
	bool exists (nano::block_hash const &) const;

public: // Tests
	entry_t back () const;

private:
	// clang-format off
	class tag_root {};
	class tag_hash {};
	class tag_sequence {};

	using ordered_recent_confirmations = boost::multi_index_container<entry_t,
	mi::indexed_by<
		mi::sequenced<mi::tag<tag_sequence>>,
		mi::hashed_unique<mi::tag<tag_root>,
			mi::member<entry_t, nano::qualified_root, &entry_t::first>>,
		mi::hashed_unique<mi::tag<tag_hash>,
			mi::member<entry_t, nano::block_hash, &entry_t::second>>>>;
	// clang-format on
	ordered_recent_confirmations confirmed;

	std::size_t const max_size;

	mutable nano::mutex mutex;

public: // Container info
	std::unique_ptr<container_info_component> collect_container_info (std::string const &);
};

/*
 * Helper container for storing recently cemented elections (a block from election might be confirmed but not yet cemented by confirmation height processor)
 */
class recently_cemented_cache final
{
public:
	using queue_t = std::deque<nano::election_status>;

	explicit recently_cemented_cache (std::size_t max_size);
	recently_cemented_cache (recently_cemented_cache &&) = delete;
	explicit recently_cemented_cache (recently_cemented_cache const &);
	~recently_cemented_cache ();
	nano::recently_cemented_cache & operator= (const nano::recently_cemented_cache &);

	void put (nano::election_status const &);
	queue_t list () const;
	std::size_t size () const;
	rsnano::RecentlyCementedCacheHandle * handle;

public: // Container info
	std::unique_ptr<container_info_component> collect_container_info (std::string const &);
};

class active_transactions;

class active_transactions_lock
{
public:
	active_transactions_lock (nano::active_transactions const & active_transactions);
	active_transactions_lock (active_transactions_lock const &) = delete;
	~active_transactions_lock ();

	void lock ();
	void unlock ();
	bool owns_lock ();

	rsnano::ActiveTransactionsLockHandle * handle;
	nano::active_transactions const & active_transactions;
};

/**
 * Core class for determining consensus
 * Holds all active blocks i.e. recently added blocks that need confirmation
 */
class active_transactions final
{
private: // Elections
	class conflict_info final
	{
	public:
		nano::qualified_root root;
		std::shared_ptr<nano::election> election;
	};

	friend class nano::election;

public:
	active_transactions (nano::node &, nano::confirmation_height_processor &, nano::block_processor &);
	active_transactions (active_transactions const &) = delete;
	~active_transactions ();

	void start ();
	void stop ();

	/**
	 * Starts new election with a specified behavior type
	 */
	nano::election_insertion_result insert (std::shared_ptr<nano::block> const & block, nano::election_behavior behavior = nano::election_behavior::normal);
	// Distinguishes replay votes, cannot be determined if the block is not in any election
	nano::vote_code vote (std::shared_ptr<nano::vote> const &);
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
	void erase (nano::block const &);
	void erase_hash (nano::block_hash const &);
	void erase_oldest ();
	bool empty () const;
	std::size_t size () const;
	bool publish (std::shared_ptr<nano::block> const &);
	boost::optional<nano::election_status_type> confirm_block (store::transaction const &, std::shared_ptr<nano::block> const &);
	boost::optional<nano::election_status_type> try_confirm (nano::election & election, nano::block_hash const & hash);
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
	std::function<void ()> vacancy_update{ [] () {} };

	std::size_t election_winner_details_size ();
	void add_election_winner_details (nano::block_hash const &, std::shared_ptr<nano::election> const &);
	void remove_election_winner_details (nano::block_hash const &);
	nano::active_transactions_lock lock () const;
	void process_confirmed (nano::election_status const & status_a, uint64_t iteration_a = 0);
	// lock_a does not own the mutex on return
	void confirm_once (nano::election_lock & lock_a, nano::election_status_type type_a, nano::election & election);
	nano::tally_t tally_impl (nano::election_lock & lock) const;
	void remove_votes (nano::election & election, nano::election_lock & lock, nano::block_hash const & hash_a);
	bool have_quorum (nano::tally_t const & tally_a) const;
	// Confirm this block if quorum is met
	void confirm_if_quorum (nano::election_lock & lock_a, nano::election & election);
	void force_confirm (nano::election & election, nano::election_status_type type_a = nano::election_status_type::active_confirmed_quorum);
	/**
	 * Calculates minimum time delay between subsequent votes when processing non-final votes
	 */
	std::chrono::seconds cooldown_time (nano::uint128_t weight) const;
	// Returns true when the winning block is durably confirmed in the ledger.
	// Later once the confirmation height processor has updated the confirmation height it will be confirmed on disk
	// It is possible for an election to be confirmed on disk but not in memory, for instance if implicitly confirmed via confirmation height
	bool confirmed (nano::election const & election) const;
	bool confirmed_locked (nano::election_lock & lock) const;
	bool confirmed (nano::block_hash const & hash) const;
	void remove_block (nano::election_lock & lock, nano::block_hash const & hash_a);
	bool replace_by_weight (nano::election & election, nano::election_lock & lock_a, nano::block_hash const & hash_a);
	std::vector<nano::vote_with_weight_info> votes_with_weight (nano::election & election) const;
	bool publish (std::shared_ptr<nano::block> const & block_a, nano::election & election);
	/*
	 * Process vote. Internally uses cooldown to throttle non-final votes
	 * If the election reaches consensus, it will be confirmed
	 */
	nano::vote_result vote (nano::election & election, nano::account const & rep, uint64_t timestamp_a, nano::block_hash const & block_hash_a, nano::vote_source vote_source_a = nano::vote_source::live);
	/**
	 * Inserts votes stored in the cache entry into this election
	 */
	std::size_t fill_from_cache (nano::election & election, nano::vote_cache::entry const & entry);
	nano::election_extended_status current_status (nano::election & election) const;
	nano::tally_t tally (nano::election & election) const;
	void on_block_confirmed (std::function<void (std::shared_ptr<nano::block> const &, nano::store::read_transaction const &, nano::election_status_type)> callback);

private:
	// Erase elections if we're over capacity
	void trim ();
	void request_loop ();
	void request_confirm (nano::active_transactions_lock &);
	void erase (nano::qualified_root const &);
	// Erase all blocks from active and, if not confirmed, clear digests from network filters
	void cleanup_election (nano::active_transactions_lock & lock_a, std::shared_ptr<nano::election>);
	nano::stat::type completion_type (nano::election const & election) const;
	// Returns a list of elections sorted by difficulty, mutex must be locked
	std::vector<std::shared_ptr<nano::election>> list_active_impl (std::size_t, nano::active_transactions_lock & guard) const;
	/**
	 * Checks if vote passes minimum representative weight threshold and adds it to inactive vote cache
	 * TODO: Should be moved to `vote_cache` class
	 */
	void add_vote_cache (nano::block_hash const & hash, std::shared_ptr<nano::vote> vote);
	boost::optional<nano::election_status_type> election_status (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block);
	void process_inactive_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block);
	void process_active_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block, nano::election_status_type status);
	void handle_final_votes_confirmation (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction, nano::election_status_type status);
	void handle_confirmation (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block, std::shared_ptr<nano::election> election, nano::election_status_type status);
	void notify_observers (nano::store::read_transaction const & transaction, nano::election_status const & status, std::vector<nano::vote_with_weight_info> const & votes);
	/**
	 * Broadcast vote for current election winner. Generates final vote if reached quorum or already confirmed
	 * Requires mutex lock
	 */
	void broadcast_vote_locked (nano::election_lock & lock, nano::election & election);
	/**
	 * Broadcasts vote for the current winner of this election
	 * Checks if sufficient amount of time (`vote_generation_interval`) passed since the last vote generation
	 */
	void broadcast_vote (nano::election & election, nano::election_lock & lock_a);
	bool broadcast_block_predicate (nano::election & election, nano::election_lock & lock_a) const;
	void broadcast_block (nano::confirmation_solicitor & solicitor_a, nano::election & election, nano::election_lock & lock_a);
	// Minimum time between broadcasts of the current winner of an election, as a backup to requesting confirmations
	std::chrono::milliseconds base_latency () const;
	/**
	 * Calculates time delay between broadcasting confirmation requests
	 */
	std::chrono::milliseconds confirm_req_time (nano::election & election) const;
	void send_confirm_req (nano::confirmation_solicitor & solicitor_a, nano::election & election, nano::election_lock & lock_a);
	bool transition_time (nano::confirmation_solicitor & solicitor_a, nano::election & election);

private: // Dependencies
	nano::node & node;
	nano::confirmation_height_processor & confirmation_height_processor;
	nano::block_processor & block_processor;

public:
	recently_confirmed_cache recently_confirmed;
	recently_cemented_cache recently_cemented;

private:
	nano::mutex election_winner_details_mutex{ mutex_identifier (mutexes::election_winner_details) };
	std::unordered_map<nano::block_hash, std::shared_ptr<nano::election>> election_winner_details;

	// Maximum time an election can be kept active if it is extending the container
	std::chrono::seconds const election_time_to_live;

	std::thread thread;
	std::function<void (std::shared_ptr<nano::block> const &, nano::store::read_transaction const &, nano::election_status_type)> block_confirmed_callback;

public:
	rsnano::ActiveTransactionsHandle * handle;

private:
	friend class election;
	friend class active_transactions_lock;
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
