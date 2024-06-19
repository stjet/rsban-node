#pragma once
#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/election_behavior.hpp>

#include <boost/optional.hpp>

#include <memory>

namespace nano
{
class block;
class node;
class container_info_component;
}

namespace nano::scheduler
{
class manual final
{
public:
	manual (rsnano::ManualSchedulerHandle * handle);
	manual (manual const &) = delete;
	~manual ();

	// Manualy start an election for a block
	// Call action with confirmed block, may be different than what we started with
	void push (std::shared_ptr<nano::block> const &, boost::optional<nano::uint128_t> const & = boost::none);

	rsnano::ManualSchedulerHandle * handle;
}; // class manual
} // nano::scheduler
