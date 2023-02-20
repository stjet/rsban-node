#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/observer_set.hpp>
#include <nano/secure/common.hpp>

#include <atomic>
#include <condition_variable>
#include <thread>

namespace nano
{
class stats;
class ledger;
class election_scheduler;

class backlog_population final
{
public:
	struct config
	{
		/** Control if ongoing backlog population is enabled. If not, backlog population can still be triggered by RPC */
		bool enabled;

		/** Number of accounts per second to process. Number of accounts per single batch is this value divided by `frequency` */
		unsigned batch_size;

		/** Number of batches to run per second. Batches run in 1 second / `frequency` intervals */
		unsigned frequency;
	};

	backlog_population (const config &, nano::ledger &, nano::stats &);
	backlog_population (backlog_population const &) = delete;
	backlog_population (backlog_population &&) = delete;
	~backlog_population ();

	void start ();
	void stop ();

	/** Manually trigger backlog population */
	void trigger ();

	/** Notify about AEC vacancy */
	void notify ();

	void set_activate_callback (std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)>);

private:
	rsnano::BacklogPopulationHandle * handle;
};
}
