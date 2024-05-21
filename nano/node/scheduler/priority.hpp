#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/node/election_status.hpp>

#include <boost/optional.hpp>

#include <condition_variable>
#include <deque>
#include <memory>
#include <string>
#include <thread>

namespace rsnano
{
class ElectionSchedulerHandle;
}

namespace nano
{
class block;
class container_info_component;
class node;
class stats;
}

namespace nano::store
{
class transaction;
class read_transaction;
}

namespace nano::scheduler
{
class priority final
{
public:
	priority (nano::node &, nano::stats &);
	priority (rsnano::ElectionSchedulerHandle * handle);
	priority (priority const &) = delete;
	priority (priority &&) = delete;
	~priority ();

	void start ();
	void stop ();

	/**
	 * Activates the first unconfirmed block of \p account_a
	 * @return true if account was activated
	 */
	bool activate (nano::account const &, store::transaction const &);
	void notify ();
	std::size_t size () const;
	bool empty () const;
	void activate_successors (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block);

	std::unique_ptr<container_info_component> collect_container_info (std::string const & name);

	rsnano::ElectionSchedulerHandle * handle;
};
}
