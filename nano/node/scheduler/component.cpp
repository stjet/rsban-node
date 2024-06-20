#include <nano/node/node.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/hinted.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/optimistic.hpp>
#include <nano/node/scheduler/priority.hpp>

nano::scheduler::component::component (rsnano::NodeHandle * handle) :
	manual_impl{ std::make_unique<nano::scheduler::manual> (rsnano::rsn_node_manual (handle)) },
	priority_impl{ std::make_unique<nano::scheduler::priority> (rsnano::rsn_node_priority (handle)) },
	manual{ *manual_impl },
	priority{ *priority_impl }
{
}

nano::scheduler::component::~component ()
{
}
