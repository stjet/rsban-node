#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/node/vote_cache.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

#include <atomic>
#include <chrono>
#include <memory>
#include <unordered_set>

namespace nano
{
class channel;
class confirmation_solicitor;
class inactive_cache_information;
class node;

class vote_info final
{
public:
	vote_info () :
		handle{ rsnano::rsn_vote_info_create1 () }
	{
	}

	vote_info (uint64_t timestamp, nano::block_hash hash) :
		handle{ rsnano::rsn_vote_info_create2 (timestamp, hash.bytes.data ()) }
	{
	}

	vote_info (rsnano::VoteInfoHandle * handle) :
		handle{ handle }
	{
	}

	vote_info (vote_info && other) :
		handle{ other.handle }
	{
		other.handle = nullptr;
	}

	vote_info (vote_info const & other) :
		handle{ rsnano::rsn_vote_info_clone (other.handle) }
	{
	}

	~vote_info ()
	{
		if (handle != nullptr)
		{
			rsnano::rsn_vote_info_destroy (handle);
		}
	}

	vote_info & operator= (vote_info const & other)
	{
		if (handle != nullptr)
		{
			rsnano::rsn_vote_info_destroy (handle);
		}
		handle = rsnano::rsn_vote_info_clone (other.handle);
		return *this;
	}

	std::chrono::system_clock::time_point get_time () const
	{
		auto value = rsnano::rsn_vote_info_time_ns (handle);
		return std::chrono::system_clock::time_point (std::chrono::duration_cast<std::chrono::system_clock::duration> (std::chrono::nanoseconds (value)));
	}

	vote_info with_relative_time (std::chrono::seconds seconds)
	{
		return { rsnano::rsn_vote_info_with_relative_time (handle, seconds.count ()) };
	}

	uint64_t get_timestamp () const
	{
		return rsnano::rsn_vote_info_timestamp (handle);
	}

	nano::block_hash get_hash () const
	{
		nano::block_hash hash;
		rsnano::rsn_vote_info_hash (handle, hash.bytes.data ());
		return hash;
	}

	rsnano::VoteInfoHandle * handle;
};

class vote_with_weight_info final
{
public:
	nano::account representative;
	std::chrono::system_clock::time_point time;
	uint64_t timestamp;
	nano::block_hash hash;
	nano::uint128_t weight;
};

class election_vote_result final
{
public:
	election_vote_result () = default;
	election_vote_result (bool, bool);
	bool replay{ false };
	bool processed{ false };
};

enum class election_behavior
{
	normal,
	/**
	 * Hinted elections:
	 * - shorter timespan
	 * - limited space inside AEC
	 */
	hinted,
	/**
	 * Optimistic elections:
	 * - shorter timespan
	 * - limited space inside AEC
	 * - more frequent confirmation requests
	 */
	optimistic,
};

nano::stat::detail to_stat_detail (nano::election_behavior);

struct election_extended_status final
{
	nano::election_status status;
	std::unordered_map<nano::account, nano::vote_info> votes;
	nano::tally_t tally;
};

class election;

class election_lock
{
public:
	election_lock (nano::election const & election);
	election_lock (election_lock const &) = delete;
	~election_lock ();
	void unlock ();
	void lock ();
	nano::election_status status () const;
	void set_status (nano::election_status status);

	void insert_or_assign_last_block (std::shared_ptr<nano::block> const & block);
	void erase_last_block (nano::block_hash const & hash);
	size_t last_blocks_size () const;
	std::unordered_map<nano::block_hash, std::shared_ptr<nano::block>> last_blocks () const;
	std::shared_ptr<nano::block> find_block (nano::block_hash const & hash);

	void insert_or_assign_vote (nano::account const & account, nano::vote_info const & vote_info);
	std::optional<nano::vote_info> find_vote (nano::account const & account) const;
	size_t last_votes_size () const;
	std::unordered_map<nano::account, nano::vote_info> last_votes () const;
	void erase_vote (nano::account const & account);
	void set_final_weight (nano::amount const & weight);
	nano::amount final_weight () const;

	nano::election & election;
	rsnano::ElectionLockHandle * handle;
};

class election_helper
{
public:
	election_helper (nano::node & node_a);
	/**
	 * Calculates minimum time delay between subsequent votes when processing non-final votes
	 */
	std::chrono::seconds cooldown_time (nano::uint128_t weight) const;
	// Minimum time between broadcasts of the current winner of an election, as a backup to requesting confirmations
	std::chrono::milliseconds base_latency () const;
	// lock_a does not own the mutex on return
	void confirm_once (nano::election_lock & lock_a, nano::election_status_type type_a, nano::election & election);
	bool confirmed (nano::election_lock & lock) const;
	// Returns true when the winning block is durably confirmed in the ledger.
	// Later once the confirmation height processor has updated the confirmation height it will be confirmed on disk
	// It is possible for an election to be confirmed on disk but not in memory, for instance if implicitly confirmed via confirmation height
	bool confirmed (nano::election & election) const;
	/**
	 * Broadcast vote for current election winner. Generates final vote if reached quorum or already confirmed
	 * Requires mutex lock
	 */
	void broadcast_vote_impl (nano::election_lock & lock, nano::election & election);

private:
	nano::node & node;
};

class election final : public std::enable_shared_from_this<nano::election>
{
public:
	enum class vote_source
	{
		live,
		cache,
	};

private:
	std::function<void (std::shared_ptr<nano::block> const &)> confirmation_action;
	std::function<void (nano::account const &)> live_vote_action;

private: // State management
	enum class state_t
	{
		passive, // only listening for incoming votes
		active, // actively request confirmations
		confirmed, // confirmed but still listening for votes
		expired_confirmed,
		expired_unconfirmed
	};

	static unsigned constexpr passive_duration_factor = 5;
	static unsigned constexpr active_request_count_min = 2;

	static_assert (std::is_trivial<std::chrono::steady_clock::duration> ());

	bool valid_change (nano::election::state_t, nano::election::state_t) const;
	bool state_change (nano::election::state_t, nano::election::state_t);

public: // State transitions
	nano::election_lock lock () const;
	bool transition_time (nano::confirmation_solicitor &, nano::election_helper & helper);
	void transition_active ();

public: // Status
	// Returns true when the election is confirmed in memory
	// Elections will first confirm in memory once sufficient votes have been received
	bool status_confirmed () const;
	bool failed () const;
	nano::election_extended_status current_status () const;
	std::shared_ptr<nano::block> winner () const;
	void log_votes (nano::election_lock & lock, nano::tally_t const &, std::string const & = "") const;
	nano::tally_t tally () const;
	bool have_quorum (nano::tally_t const &) const;
	unsigned get_confirmation_request_count () const;
	void inc_confirmation_request_count ();

public: // Interface
	election (nano::node &, std::shared_ptr<nano::block> const & block, std::function<void (std::shared_ptr<nano::block> const &)> const & confirmation_action, std::function<void (nano::account const &)> const & vote_action, nano::election_behavior behavior);
	election (election const &) = delete;
	election (election &&) = delete;
	~election ();

	std::shared_ptr<nano::block> find (nano::block_hash const &) const;
	/*
	 * Process vote. Internally uses cooldown to throttle non-final votes
	 * If the election reaches consensus, it will be confirmed
	 */
	nano::election_vote_result vote (nano::election_helper & helper, nano::account const & representative, uint64_t timestamp, nano::block_hash const & block_hash, vote_source = vote_source::live);
	/**
	* Inserts votes stored in the cache entry into this election
	*/
	std::size_t fill_from_cache (nano::election_helper & helper, nano::vote_cache::entry const & entry);

	bool publish (std::shared_ptr<nano::block> const & block_a, nano::election_helper & helper);
	// Confirm this block if quorum is met
	void confirm_if_quorum (nano::election_lock &, nano::election_helper &);
	boost::optional<nano::election_status_type> try_confirm (nano::block_hash const & hash, nano::election_helper & helper);
	void set_status_type (nano::election_status_type status_type);

	/**
	 * Broadcasts vote for the current winner of this election
	 * Checks if sufficient amount of time (`vote_generation_interval`) passed since the last vote generation
	 */
	void broadcast_vote (nano::election_helper & helper);
	nano::vote_info get_last_vote (nano::account const & account);
	void set_last_vote (nano::account const & account, nano::vote_info vote_info);
	nano::election_status get_status () const;
	void set_status (nano::election_status status_a);

private: // Dependencies
	nano::node & node;

public: // Information
	nano::root root () const;
	nano::qualified_root qualified_root () const;
	std::vector<nano::vote_with_weight_info> votes_with_weight () const;
	nano::election_behavior behavior () const;

private:
	nano::tally_t tally_impl (nano::election_lock & lock) const;
	void broadcast_block (nano::confirmation_solicitor &, nano::election_helper &);
	void send_confirm_req (nano::confirmation_solicitor &, nano::election_helper &);
	void remove_votes (nano::election_lock & lock, nano::block_hash const &);
	void remove_block (nano::election_lock & lock, nano::block_hash const &);
	bool replace_by_weight (nano::election_lock & lock_a, nano::block_hash const &);
	std::chrono::milliseconds time_to_live () const;
	/**
	 * Calculates time delay between broadcasting confirmation requests
	 */
	std::chrono::milliseconds confirm_req_time (nano::election_helper & helper) const;
	bool is_quorum () const;

private: // Constants
	static std::size_t constexpr max_blocks{ 10 };

	friend class active_transactions;
	friend class confirmation_solicitor;
	friend class election_helper;

public: // Only used in tests
	void force_confirm (nano::election_helper & helper, nano::election_status_type = nano::election_status_type::active_confirmed_quorum);
	std::unordered_map<nano::account, nano::vote_info> votes () const;
	std::unordered_map<nano::block_hash, std::shared_ptr<nano::block>> blocks () const;

	friend class confirmation_solicitor_different_hash_Test;
	friend class confirmation_solicitor_bypass_max_requests_cap_Test;
	friend class votes_add_existing_Test;
	friend class votes_add_old_Test;
	rsnano::ElectionHandle * handle;
};
}
