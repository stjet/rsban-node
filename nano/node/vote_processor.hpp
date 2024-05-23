#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <deque>
#include <memory>
#include <thread>

namespace nano
{
class signature_checker;
class active_transactions;
namespace store
{
	class component;
}
class node_observers;
class stats;
class node_config;
class logger;
class online_reps;
class rep_crawler;
class ledger;
class network_params;
class node_flags;
class stats;
class rep_tiers;

namespace transport
{
	class channel;
}

class vote_processor_queue
{
public:
	vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, nano::rep_tiers & rep_tiers_a);
	vote_processor_queue (rsnano::VoteProcessorQueueHandle * handle);
	vote_processor_queue (vote_processor_queue const &) = delete;
	~vote_processor_queue ();

	std::size_t size () const;
	bool empty () const;
	/** Returns false if the vote was processed */
	bool vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a);
	/** Function blocks until the queue is empty */
	void flush ();
	void stop ();

	rsnano::VoteProcessorQueueHandle * handle;
};

std::unique_ptr<container_info_component> collect_container_info (vote_processor_queue & queue, std::string const & name);

class vote_processor final
{
public:
	vote_processor (rsnano::VoteProcessorHandle * handle);
	vote_processor (
	nano::vote_processor_queue & queue_a,
	nano::active_transactions & active_a,
	std::shared_ptr<nano::node_observers> observers_a,
	nano::stats & stats_a,
	nano::node_config & config_a,
	nano::logger & logger_a,
	nano::rep_crawler & rep_crawler_a,
	nano::network_params & network_params_a,
	nano::rep_tiers & rep_tiers_a);

	vote_processor (vote_processor const &) = delete;
	~vote_processor ();

	void start ();
	void stop ();

	/** Note: node.active.mutex lock is required */
	nano::vote_code vote_blocking (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &, bool = false);

	uint64_t total_processed () const;

	rsnano::VoteProcessorHandle * handle;
};

}
