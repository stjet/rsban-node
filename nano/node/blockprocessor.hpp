#pragma once

#include <nano/node/transport/channel.hpp>
#include <nano/secure/common.hpp>

#include <chrono>
#include <functional>
#include <future>
#include <memory>
#include <optional>
#include <thread>

namespace nano
{
class block;
class node;
class write_database_queue;
class logger;
}

namespace nano::store
{
class write_transaction;
}

namespace nano
{
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

std::string_view to_string (block_source);

class block_processor_config final
{
public:
	block_processor_config () = default;
	explicit block_processor_config (rsnano::BlockProcessorConfigDto const &);

	nano::error deserialize (nano::tomlconfig & toml);
	rsnano::BlockProcessorConfigDto to_dto () const;

public:
	// Maximum number of blocks to queue from network peers
	size_t max_peer_queue;
	// Maximum number of blocks to queue from system components (local RPC, bootstrap)
	size_t max_system_queue;

	// Higher priority gets processed more frequently
	size_t priority_live;
	size_t priority_bootstrap;
	size_t priority_local;
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
		context (std::shared_ptr<block> block, block_source source);
		explicit context (rsnano::BlockProcessorContextHandle * handle_a);
		context (context const &) = delete;
		context (context &&);
		~context ();

		block_source const source{};
		std::shared_ptr<nano::block> get_block () const;

	public:
		using result_t = nano::block_status;
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

	std::size_t size () const;
	std::size_t size (block_source) const;
	bool full () const;
	bool half_full () const;
	void process_active (std::shared_ptr<nano::block> const & incoming);
	bool add (std::shared_ptr<nano::block> const &, block_source = block_source::live, std::shared_ptr<nano::transport::channel> const & channel = nullptr);
	std::optional<nano::block_status> add_blocking (std::shared_ptr<nano::block> const & block, block_source);
	void force (std::shared_ptr<nano::block> const &);
	bool flushing ();

	std::unique_ptr<nano::container_info_component> collect_container_info (std::string const & name);

	rsnano::BlockProcessorHandle const * get_handle () const;

public: // Events
	using processed_t = std::tuple<nano::block_status, context>;
	using processed_batch_t = std::deque<processed_t>;

	void set_blocks_rolled_back_callback (std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> callback);

	// The batch observer feeds the processed observer
	nano::observer_set<nano::block_status const &, context const &> block_processed;
	nano::observer_set<processed_batch_t const &> batch_processed;
	nano::observer_set<std::shared_ptr<nano::block> const &> rolled_back;

private:
	void run ();
	processed_batch_t process_batch (nano::block_processor_lock &);

	bool stopped{ false };
	nano::stats & stats;
	nano::logger & logger;
	nano::node_config & config; // already ported
	nano::network_params & network_params; // already ported

public:
	rsnano::BlockProcessorHandle * handle;

private:
	nano::node_flags & flags; // already ported
	std::thread thread;

	friend std::unique_ptr<container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
	friend class block_processor_lock;
};
}
