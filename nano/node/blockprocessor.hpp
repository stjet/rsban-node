#pragma once

#include <nano/node/transport/channel.hpp>
#include <nano/secure/common.hpp>

#include <functional>
#include <future>
#include <memory>
#include <optional>
#include <thread>
#include <vector>

namespace nano
{
class block;
class node;
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
	block_processor (nano::node &);
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
	using processed_batch_t = std::vector<std::tuple<nano::block_status, std::shared_ptr<nano::block>, nano::block_source>>;

	void set_blocks_rolled_back_callback (std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> callback);

	// The batch observer feeds the processed observer
	void add_block_processed_observer (std::function<void (nano::block_status, std::shared_ptr<nano::block> const &, nano::block_source)> observer);
	void add_batch_processed_observer (std::function<void (processed_batch_t const &)> observer);
	void add_rolled_back_observer (std::function<void (std::shared_ptr<nano::block> const &)> observer);
	void notify_block_rolled_back (std::shared_ptr<nano::block> const & block);

public:
	rsnano::BlockProcessorHandle * handle;

private:
	std::thread thread;

	friend std::unique_ptr<container_info_component> collect_container_info (block_processor & block_processor, std::string const & name);
	friend class block_processor_lock;
};
}
