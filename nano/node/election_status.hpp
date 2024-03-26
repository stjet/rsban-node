#pragma once

#include <nano/lib/numbers.hpp>

#include <chrono>
#include <memory>

namespace rsnano
{
class ElectionStatusHandle;
}
namespace nano
{
class block;
}

namespace nano
{
/* Defines the possible states for an election to stop in */
enum class election_status_type : uint8_t
{
	ongoing = 0,
	active_confirmed_quorum = 1,
	active_confirmation_height = 2,
	inactive_confirmation_height = 3,
	stopped = 5
};

/* Holds a summary of an election */
class election_status final
{
public:
	election_status ();
	election_status (rsnano::ElectionStatusHandle * handle);
	election_status (std::shared_ptr<nano::block> const & winner_a);
	election_status (election_status &&);
	election_status (election_status const &);
	~election_status ();
	nano::election_status & operator= (const nano::election_status &);
	std::shared_ptr<nano::block> get_winner () const;
	nano::amount get_tally () const;
	nano::amount get_final_tally () const;
	std::chrono::milliseconds get_election_end () const;
	std::chrono::milliseconds get_election_duration () const;
	unsigned get_confirmation_request_count () const;
	unsigned get_block_count () const;
	unsigned get_voter_count () const;
	election_status_type get_election_status_type () const;
	void set_winner (std::shared_ptr<nano::block>);
	void set_tally (nano::amount);
	void set_final_tally (nano::amount);
	void set_election_end (std::chrono::milliseconds);
	void set_election_duration (std::chrono::milliseconds);
	void set_confirmation_request_count (uint32_t);
	void set_block_count (uint32_t);
	void set_voter_count (uint32_t);
	void set_election_status_type (nano::election_status_type);
	rsnano::ElectionStatusHandle * handle;
};
}
