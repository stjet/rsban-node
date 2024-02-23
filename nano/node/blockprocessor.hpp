#pragma once

#include "nano/node/websocket.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/secure/common.hpp>

#include <chrono>
#include <functional>
#include <future>
#include <memory>
#include <optional>
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

enum class block_source
{
	unknown = 0,
	live,
	bootstrap,
	bootstrap_legacy,
	unchecked,
	local,
	forced,
};

/**
 * Processing blocks is a potentially long IO operation.
 * This class isolates block insertion from other operations like servicing network operations
 */
class block_processor final
{
public: // Context
	class context
	{
	public:
		explicit context (block_source);
		explicit context (rsnano::BlockProcessorContextHandle * handle_a);
		context (context const &) = delete;
		context (context &&);
		~context ();

		block_source const source{};

	public:
		using result_t = nano::process_return;
		std::future<result_t> get_future ();

	private:
		void set_result (result_t const &);

		friend class block_processor;

	public:
		rsnano::BlockProcessorContextHandle * handle;
	};

public:
	block_processor (nano::node &, nano::write_database_queue &);
	block_processor (nano::block_processor const &) = delete;
	block_processor (nano::block_processor &&) = delete;
	~block_processor ();
	void start ();
	void stop ();
	std::size_t size ();
	bool full ();
	bool half_full ();
	void process_active (std::shared_ptr<nano::block> const & incoming);
	void add (std::shared_ptr<nano::block> const &, block_source = block_source::live);
	std::optional<nano::process_return> add_blocking (std::shared_ptr<nano::block> const & block, block_source);
	void force (std::shared_ptr<nano::block> const &);
	bool have_blocks_ready (nano::block_processor_lock & lock_a);
	bool have_blocks (nano::block_processor_lock & lock_a);
	void process_blocks ();
	bool flushing ();

	rsnano::BlockProcessorHandle const * get_handle () const;

public: // Events
	using processed_t = std::tuple<nano::process_return, std::shared_ptr<nano::block>, context>;
	using processed_batch_t = std::deque<processed_t>;

	void set_blocks_rolled_back_callback (std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> callback);

	// The batch observer feeds the processed obsever
	nano::observer_set<nano::process_return const &, std::shared_ptr<nano::block> const &, context const &> processed;
	nano::observer_set<processed_batch_t const &> batch_processed;

private:
	processed_batch_t process_batch (nano::block_processor_lock &);

	bool stopped{ false };
	bool active{ false };

	nano::stats & stats;
	nano::node_config & config; // already ported
	nano::network_params & network_params; // already ported

public:
	rsnano::BlockProcessorHandle * handle;

private:
	nano::node_flags & flags; // already ported
	std::thread processing_thread;

	friend std::unique_ptr<container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
	friend class block_processor_lock;
};

std::unique_ptr<nano::container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
}
