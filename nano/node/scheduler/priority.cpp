#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>

nano::scheduler::priority::priority (rsnano::ElectionSchedulerHandle * handle) :
	handle{ handle }
{
}

nano::scheduler::priority::~priority ()
{
	rsnano::rsn_election_scheduler_destroy (handle);
}

bool nano::scheduler::priority::activate (store::transaction const & transaction, nano::account const & account_a)
{
	return rsnano::rsn_election_scheduler_activate (handle, account_a.bytes.data (), transaction.get_rust_handle ());
}

std::size_t nano::scheduler::priority::size () const
{
	return rsnano::rsn_election_scheduler_len (handle);
}

bool nano::scheduler::priority::empty () const
{
	return rsnano::rsn_election_scheduler_empty (handle);
}
