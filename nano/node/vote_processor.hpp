#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <deque>
#include <memory>
#include <thread>
#include <unordered_set>

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
class logger_mt;
class online_reps;
class rep_crawler;
class ledger;
class network_params;
class node_flags;
class stats;

namespace transport
{
	class channel;
}

class vote_processor_queue
{
public:
	vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, std::shared_ptr<nano::logger_mt> & logger_a);
	vote_processor_queue (vote_processor_queue const &) = delete;
	~vote_processor_queue ();

	std::size_t size ();
	bool empty ();
	/** Returns false if the vote was processed */
	bool vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a);
	void calculate_weights ();
	bool wait_and_take (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> & votes_a);
	/** Function blocks until the queue is empty */
	void flush ();
	void clear ();
	void stop ();

	rsnano::VoteProcessorQueueHandle * handle;
};

std::unique_ptr<container_info_component> collect_container_info (vote_processor_queue & queue, std::string const & name);

class vote_processor final
{
public:
	vote_processor (
	nano::vote_processor_queue & queue_a,
	nano::active_transactions & active_a,
	nano::node_observers & observers_a,
	nano::stats & stats_a,
	nano::node_config & config_a,
	nano::logger_mt & logger_a,
	nano::rep_crawler & rep_crawler_a,
	nano::network_params & network_params_a);

	/** Note: node.active.mutex lock is required */
	nano::vote_code vote_blocking (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &, bool = false);
	void verify_votes (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> const &);
	void stop ();
	std::atomic<uint64_t> total_processed{ 0 };

	void process_loop ();

	nano::active_transactions & active;
	nano::node_observers & observers;
	nano::stats & stats;
	nano::node_config & config;
	nano::logger_mt & logger;
	nano::rep_crawler & rep_crawler;
	nano::network_params & network_params;
	bool started;
	std::thread thread;
	nano::condition_variable condition;
	nano::mutex mutex{ mutex_identifier (mutexes::vote_processor) };

public:
	nano::vote_processor_queue & queue;

	friend class vote_processor_weights_Test;
};

}
