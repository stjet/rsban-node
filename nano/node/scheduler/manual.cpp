#include "nano/lib/rsnano.hpp"

#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/manual.hpp>

nano::scheduler::manual::manual (nano::node & node) :
	handle{ rsnano::rsn_manual_scheduler_create (node.stats->handle, node.active.handle) }
{
}

nano::scheduler::manual::manual (rsnano::ManualSchedulerHandle * handle) :
	handle{ handle }
{
}

nano::scheduler::manual::~manual ()
{
	rsnano::rsn_manual_scheduler_destroy (handle);
}

void nano::scheduler::manual::start ()
{
	rsnano::rsn_manual_scheduler_start (handle);
}

void nano::scheduler::manual::stop ()
{
	rsnano::rsn_manual_scheduler_stop (handle);
}

void nano::scheduler::manual::push (std::shared_ptr<nano::block> const & block_a, boost::optional<nano::uint128_t> const & previous_balance_a, nano::election_behavior election_behavior_a)
{
	uint8_t * previous_ptr = nullptr;
	nano::amount amount;
	if (previous_balance_a.has_value ())
	{
		amount = previous_balance_a.value ();
		previous_ptr = amount.bytes.data ();
	}
	rsnano::rsn_manual_scheduler_push (handle, block_a->get_handle (), previous_ptr, static_cast<uint8_t> (election_behavior_a));
}
