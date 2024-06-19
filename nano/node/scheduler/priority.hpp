#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/node/election_status.hpp>
#include <boost/optional.hpp>

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
	priority (rsnano::ElectionSchedulerHandle * handle);
	priority (priority const &) = delete;
	priority (priority &&) = delete;
	~priority ();

	/**
	 * Activates the first unconfirmed block of \p account_a
	 * @return true if account was activated
	 */
	bool activate (store::transaction const &, nano::account const &);
	std::size_t size () const;
	bool empty () const;

	rsnano::ElectionSchedulerHandle * handle;
};
}
