#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/election_status.hpp>

nano::election_status::election_status () :
	handle (rsnano::rsn_election_status_create ())
{
}

nano::election_status::election_status (rsnano::ElectionStatusHandle * handle_a) :
	handle (handle_a)
{
}

nano::election_status::election_status (std::shared_ptr<nano::block> const & winner_a) :
	handle (rsnano::rsn_election_status_create1 (winner_a->get_handle ()))
{
}

nano::election_status::election_status (nano::election_status && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::election_status::election_status (nano::election_status const & other_a) :
	handle (rsnano::rsn_election_status_clone (other_a.handle))
{
}

nano::election_status::~election_status ()
{
	if (handle != nullptr)
		rsnano::rsn_election_status_destroy (handle);
}

nano::election_status & nano::election_status::operator= (const nano::election_status & other_a)
{
	if (handle != nullptr)
		rsnano::rsn_election_status_destroy (handle);

	handle = rsnano::rsn_election_status_clone (other_a.handle);
	return *this;
}

std::shared_ptr<nano::block> nano::election_status::get_winner () const
{
	auto block_handle = rsnano::rsn_election_status_get_winner (handle);
	return block_handle_to_block (block_handle);
}

nano::amount nano::election_status::get_tally () const
{
	nano::amount tally;
	rsnano::rsn_election_status_get_tally (handle, tally.bytes.data ());
	return tally;
}

nano::amount nano::election_status::get_final_tally () const
{
	nano::amount final_tally;
	rsnano::rsn_election_status_get_final_tally (handle, final_tally.bytes.data ());
	return final_tally;
}

std::chrono::milliseconds nano::election_status::get_election_end () const
{
	return std::chrono::milliseconds (rsnano::rsn_election_status_get_election_end (handle));
}

std::chrono::milliseconds nano::election_status::get_election_duration () const
{
	return std::chrono::milliseconds (rsnano::rsn_election_status_get_election_duration (handle));
}

unsigned nano::election_status::get_confirmation_request_count () const
{
	return rsnano::rsn_election_status_get_confirmation_request_count (handle);
}

unsigned nano::election_status::get_block_count () const
{
	return rsnano::rsn_election_status_get_block_count (handle);
}

unsigned nano::election_status::get_voter_count () const
{
	return rsnano::rsn_election_status_get_vote_count (handle);
}

nano::election_status_type nano::election_status::get_election_status_type () const
{
	return static_cast<nano::election_status_type> (rsnano::rsn_election_status_get_election_status_type (handle));
}

void nano::election_status::set_winner (std::shared_ptr<nano::block> winner)
{
	auto block_handle = winner == nullptr ? nullptr : winner->get_handle ();
	rsnano::rsn_election_status_set_winner (handle, block_handle);
}

void nano::election_status::set_tally (nano::amount tally)
{
	rsnano::rsn_election_status_set_tally (handle, tally.bytes.data ());
}

void nano::election_status::set_final_tally (nano::amount final_tally)
{
	rsnano::rsn_election_status_set_final_tally (handle, final_tally.bytes.data ());
}

void nano::election_status::set_block_count (uint32_t block_count)
{
	rsnano::rsn_election_status_set_block_count (handle, block_count);
}

void nano::election_status::set_voter_count (uint32_t voter_count)
{
	rsnano::rsn_election_status_set_voter_count (handle, voter_count);
}

void nano::election_status::set_confirmation_request_count (uint32_t confirmation_request_count)
{
	rsnano::rsn_election_status_set_confirmation_request_count (handle, confirmation_request_count);
}

void nano::election_status::set_election_end (std::chrono::milliseconds election_end)
{
	rsnano::rsn_election_status_set_election_end (handle, election_end.count ());
}

void nano::election_status::set_election_duration (std::chrono::milliseconds election_duration)
{
	rsnano::rsn_election_status_set_election_duration (handle, election_duration.count ());
}

void nano::election_status::set_election_status_type (nano::election_status_type election_status_type)
{
	rsnano::rsn_election_status_set_election_status_type (handle, static_cast<uint8_t> (election_status_type));
}
