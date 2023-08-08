#pragma once

#include "nano/node/websocket.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/node/blocking_observer.hpp>
#include <nano/node/state_block_signature_verification.hpp>
#include <nano/secure/common.hpp>

#include <chrono>
#include <functional>
#include <future>
#include <memory>
#include <thread>

namespace nano
{
class node;
class read_transaction;
class transaction;
class write_transaction;
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
	void add (std::shared_ptr<nano::block> const &);
	std::optional<nano::process_return> add_blocking (std::shared_ptr<nano::block> const & block);
	void force (std::shared_ptr<nano::block> const &);
	bool should_log ();
	bool have_blocks_ready ();
	bool have_blocks ();
	void process_blocks ();

	std::atomic<bool> flushing{ false };
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
	void rollback_competitor (nano::write_transaction const & transaction, nano::block const & block);
	nano::process_return process_one (nano::write_transaction const &, std::shared_ptr<nano::block> block, bool const = false);
	void queue_unchecked (nano::write_transaction const &, nano::hash_or_account const &);
	std::deque<processed_t> process_batch (nano::block_processor_lock &);
	void process_verified_state_blocks (std::deque<nano::state_block_signature_verification::value_type> &, std::vector<int> const &, std::vector<nano::block_hash> const &, std::vector<nano::signature> const &);
	void add_impl (std::shared_ptr<nano::block> block);
	bool stopped{ false };
	bool active{ false };
	std::chrono::steady_clock::time_point next_log;
	std::deque<std::shared_ptr<nano::block>> blocks;
	std::deque<std::shared_ptr<nano::block>> forced;

	nano::logger_mt & logger; // already ported
	nano::signature_checker & checker; // already ported
	nano::node_config & config; // already ported
	nano::state_block_signature_verification state_block_signature_verification; // already ported
	nano::network_params & network_params; // already ported
	nano::block_arrival & block_arrival; // already ported

	rsnano::BlockProcessorHandle * handle;

	nano::ledger & ledger; // already ported
	nano::node_flags & flags; // already ported
	nano::store & store; // already ported
	nano::stats & stats; // already ported
	nano::unchecked_map & unchecked; // already ported
	nano::gap_cache & gap_cache; // already ported
	nano::write_database_queue & write_database_queue; // already ported
	std::thread processing_thread;
	std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> blocks_rolled_back;

	friend std::unique_ptr<container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
};
std::unique_ptr<nano::container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
}
