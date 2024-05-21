#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>

nano::scheduler::priority::priority (nano::node & node_a, nano::stats & stats_a) :
	handle{ rsnano::rsn_election_scheduler_create (node_a.ledger.handle, stats_a.handle, node_a.active.handle) }
{
}

nano::scheduler::priority::priority (rsnano::ElectionSchedulerHandle * handle) :
	handle{handle}
{
}

nano::scheduler::priority::~priority ()
{
	rsnano::rsn_election_scheduler_destroy (handle);
}

void nano::scheduler::priority::start ()
{
	rsnano::rsn_election_scheduler_start (handle);
}

void nano::scheduler::priority::stop ()
{
	rsnano::rsn_election_scheduler_stop (handle);
}

bool nano::scheduler::priority::activate (nano::account const & account_a, store::transaction const & transaction)
{
	return rsnano::rsn_election_scheduler_activate (handle, account_a.bytes.data (), transaction.get_rust_handle ());
}

void nano::scheduler::priority::notify ()
{
	rsnano::rsn_election_scheduler_notify (handle);
}

std::size_t nano::scheduler::priority::size () const
{
	return rsnano::rsn_election_scheduler_len (handle);
}

bool nano::scheduler::priority::empty () const
{
	return rsnano::rsn_election_scheduler_empty (handle);
}

void nano::scheduler::priority::activate_successors (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block)
{
	rsnano::rsn_election_scheduler_activate_successors (handle, transaction.get_rust_handle (), block->get_handle ());
}

std::unique_ptr<nano::container_info_component> nano::scheduler::priority::collect_container_info (std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_election_scheduler_collect_container_info (handle, name.c_str ()));
}
