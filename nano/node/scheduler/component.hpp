#pragma once

#include <nano/lib/locks.hpp>
#include <nano/node/election.hpp>

#include <memory>
#include <string>

namespace nano
{
class container_info_component;
class node;
class block;
namespace store
{
	class read_transaction;
}
}
namespace nano::scheduler
{
class hinted;
class manual;
class optimistic;
class priority;

class component final
{
	std::unique_ptr<nano::scheduler::hinted> hinted_impl;
	std::unique_ptr<nano::scheduler::manual> manual_impl;
	std::unique_ptr<nano::scheduler::optimistic> optimistic_impl;
	std::unique_ptr<nano::scheduler::priority> priority_impl;
	nano::mutex mutex;

public:
	explicit component (nano::node & node);
	~component ();

	// Starts all schedulers
	void start ();
	// Stops all schedulers
	void stop ();

	std::unique_ptr<container_info_component> collect_container_info (std::string const & name);

	nano::scheduler::hinted & hinted;
	nano::scheduler::manual & manual;
	nano::scheduler::optimistic & optimistic;
	nano::scheduler::priority & priority;
};

class successor_scheduler
{
public:
	successor_scheduler (nano::node & node);
	void schedule (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction, nano::election_status_type status);

private:
	void activate_successors (const nano::account & account, std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction);

	nano::node & node;
};
}
