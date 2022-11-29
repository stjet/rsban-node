#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/inactive_cache_status.hpp>

nano::inactive_cache_status::inactive_cache_status () :
	handle (rsnano::rsn_inactive_cache_status_create ())
{
}

nano::inactive_cache_status::inactive_cache_status (rsnano::InactiveCacheStatusHandle * handle_a) :
	handle{ handle_a }
{
}

nano::inactive_cache_status::~inactive_cache_status ()
{
	rsnano::rsn_inactive_cache_status_destroy (handle);
}

bool nano::inactive_cache_status::get_bootstrap_started () const
{
	return rsnano::rsn_inactive_cache_status_bootstrap_started (handle);
}

bool nano::inactive_cache_status::get_election_started () const
{
	return rsnano::rsn_inactive_cache_status_election_started (handle);
}

bool nano::inactive_cache_status::get_confirmed () const
{
	return rsnano::rsn_inactive_cache_status_confirmed (handle);
}

nano::uint128_t nano::inactive_cache_status::get_tally () const
{
	nano::amount tally;
	rsnano::rsn_inactive_cache_status_tally (handle, tally.bytes.data ());
	return tally.number ();
}

void nano::inactive_cache_status::set_bootstrap_started (bool bootstrap_started) const
{
	rsnano::rsn_inactive_cache_status_set_bootstrap_started (handle, bootstrap_started);
}

void nano::inactive_cache_status::set_election_started (bool election_started) const
{
	rsnano::rsn_inactive_cache_status_set_election_started (handle, election_started);
}

void nano::inactive_cache_status::set_confirmed (bool confirmed) const
{
	rsnano::rsn_inactive_cache_status_set_confirmed (handle, confirmed);
}

void nano::inactive_cache_status::set_tally (nano::uint128_t tally) const
{
	auto tally_amount = nano::amount (tally);
	rsnano::rsn_inactive_cache_status_set_tally (handle, tally_amount.bytes.data ());
}

bool nano::inactive_cache_status::operator!= (inactive_cache_status const other) const
{
	return !rsnano::rsn_inactive_cache_status_eq (handle, other.handle);
}

std::string nano::inactive_cache_status::to_string () const
{
	rsnano::StringDto result;
	rsnano::rsn_inactive_cache_status_to_string (handle, &result);
	return rsnano::convert_dto_to_string (result);
}
