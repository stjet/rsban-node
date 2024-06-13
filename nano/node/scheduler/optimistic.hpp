#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/timer.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

namespace nano
{
class account_info;
class active_elections;
class ledger;
class node;
}

namespace nano::scheduler
{
class optimistic_config final
{
public:
	optimistic_config ();
	nano::error deserialize (nano::tomlconfig & toml);
	void load_dto (rsnano::OptimisticSchedulerConfigDto const & dto_a);
	rsnano::OptimisticSchedulerConfigDto into_dto () const;

public:
	bool enabled;

	/** Minimum difference between confirmation frontier and account frontier to become a candidate for optimistic confirmation */
	std::size_t gap_threshold;

	/** Maximum number of candidates stored in memory */
	std::size_t max_size;
};

class optimistic final
{
	struct entry;

public:
	optimistic (optimistic_config const &, nano::node &, nano::ledger &, nano::active_elections &, nano::network_constants const & network_constants, nano::stats &);
	optimistic (rsnano::OptimisticSchedulerHandle * handle);
	optimistic (optimistic const &) = delete;
	~optimistic ();

	void start ();
	void stop ();

	/**
	 * Notify about changes in AEC vacancy
	 */
	void notify ();

	rsnano::OptimisticSchedulerHandle * handle;
};
}
