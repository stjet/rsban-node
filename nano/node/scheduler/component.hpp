#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/node/election.hpp>

#include <memory>

namespace nano::scheduler
{
class manual;
class priority;

class component final
{
	std::unique_ptr<nano::scheduler::manual> manual_impl;
	std::unique_ptr<nano::scheduler::priority> priority_impl;

public:
	explicit component (rsnano::NodeHandle *);
	~component ();

	nano::scheduler::manual & manual;
	nano::scheduler::priority & priority;
};
}
