#include <nano/node/node.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/hinted.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/optimistic.hpp>
#include <nano/node/scheduler/priority.hpp>

nano::scheduler::component::component (rsnano::NodeHandle * handle) :
	hinted_impl{ std::make_unique<nano::scheduler::hinted> (rsnano::rsn_node_hinted (handle)) },
	manual_impl{ std::make_unique<nano::scheduler::manual> (rsnano::rsn_node_manual (handle)) },
	optimistic_impl{ std::make_unique<nano::scheduler::optimistic> (rsnano::rsn_node_optimistic (handle)) },
	priority_impl{ std::make_unique<nano::scheduler::priority> (rsnano::rsn_node_priority (handle)) },
	hinted{ *hinted_impl },
	manual{ *manual_impl },
	optimistic{ *optimistic_impl },
	priority{ *priority_impl }
{
}

nano::scheduler::component::~component ()
{
}
