#pragma once

#include "nano/node/websocket.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/node/blocking_observer.hpp>
#include <nano/secure/common.hpp>

#include <chrono>
#include <functional>
#include <future>
#include <memory>
#include <thread>

namespace nano::store
{
class write_transaction;
}

namespace nano
{
class node;
class write_database_queue;
class node_config;
class ledger;
class node_flags;
class network;
class stats;
class local_vote_history;
class active_transactions;
class election_scheduler;
class block_arrival;
class unchecked_map;
class gap_cache;
class bootstrap_initiator;
class vote_cache;
class signature_checker;

namespace websocket
{
	class listener;
}

class block_processor_lock;

/**
 * Processing blocks is a potentially long IO operation.
 * This class isolates block insertion from other operations like servicing network operations
 */
class block_processor final
{
public:
	explicit block_processor (nano::node &, nano::write_database_queue &);
	block_processor (nano::block_processor const &) = delete;
	block_processor (nano::block_processor &&) = delete;
	~block_processor ();
	void start ();
	void stop ();
	void flush ();
	std::size_t size ();
	bool full ();
	bool half_full ();
	void process_active (std::shared_ptr<nano::block> const & incoming);
	void add (std::shared_ptr<nano::block> const &);
	std::optional<nano::process_return> add_blocking (std::shared_ptr<nano::block> const & block);
	void force (std::shared_ptr<nano::block> const &);
	bool have_blocks_ready (nano::block_processor_lock & lock_a);
	bool have_blocks (nano::block_processor_lock & lock_a);
	void process_blocks ();
	bool flushing ();

	rsnano::BlockProcessorHandle const * get_handle () const;

public: // Events
	using processed_t = std::pair<nano::process_return, std::shared_ptr<nano::block>>;
	nano::observer_set<nano::process_return const &, std::shared_ptr<nano::block>> processed;

	// The batch observer feeds the processed obsever
	nano::observer_set<std::deque<processed_t> const &> batch_processed;

private:
	blocking_observer blocking;

private:
	// Roll back block in the ledger that conflicts with 'block'
	void rollback_competitor (nano::store::write_transaction const & transaction, nano::block const & block);
	nano::process_return process_one (nano::store::write_transaction const &, std::shared_ptr<nano::block> block, bool const = false);
	void queue_unchecked (nano::store::write_transaction const &, nano::hash_or_account const &);
	std::deque<processed_t> process_batch (nano::block_processor_lock &);

	bool stopped{ false };
	bool active{ false };

	nano::logger_mt & logger; // already ported
	nano::signature_checker & checker; // already ported
	nano::node_config & config; // already ported
	nano::network_params & network_params; // already ported
	nano::block_arrival & block_arrival; // already ported

public:
	rsnano::BlockProcessorHandle * handle;

private:
	nano::ledger & ledger; // already ported
	nano::node_flags & flags; // already ported
	nano::store::component & store; // already ported
	nano::stats & stats; // already ported
	nano::unchecked_map & unchecked; // already ported
	nano::gap_cache & gap_cache; // already ported
	nano::write_database_queue & write_database_queue; // already ported
	std::thread processing_thread;
	std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> blocks_rolled_back;

	friend std::unique_ptr<container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
	friend class block_processor_lock;
};
std::unique_ptr<nano::container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
}
