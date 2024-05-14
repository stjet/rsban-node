#pragma once

#include <nano/lib/config.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/thread_pool.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/backlog_population.hpp>
#include <nano/node/bandwidth_limiter.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_server.hpp>
#include <nano/node/bootstrap_ascending/service.hpp>
#include <nano/node/confirming_set.hpp>
#include <nano/node/distributed_work_factory.hpp>
#include <nano/node/election.hpp>
#include <nano/node/local_block_broadcaster.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/online_reps.hpp>
#include <nano/node/portmapping.hpp>
#include <nano/node/process_live_dispatcher.hpp>
#include <nano/node/rep_tiers.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/node/request_aggregator.hpp>
#include <nano/node/telemetry.hpp>
#include <nano/node/transport/tcp_server.hpp>
#include <nano/node/unchecked_map.hpp>
#include <nano/node/vote_cache.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/vote_processor.hpp>
#include <nano/node/wallet.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/utility.hpp>

#include <boost/program_options.hpp>
#include <boost/thread/latch.hpp>

#include <atomic>
#include <memory>
#include <optional>
#include <vector>

namespace nano
{
class node;
class work_pool;

namespace scheduler
{
	class component;
}
namespace transport
{
	class tcp_listener;
}

// Configs
backlog_population::config backlog_population_config (node_config const &);
outbound_bandwidth_limiter::config outbound_bandwidth_limiter_config (node_config const &);

class node final : public std::enable_shared_from_this<nano::node>
{
public:
	node (rsnano::async_runtime & async_rt_a, uint16_t peering_port, std::filesystem::path const & application_path, nano::work_pool &, nano::node_flags = nano::node_flags (), unsigned seq = 0);
	node (rsnano::async_runtime & async_rt_a, std::filesystem::path const & application_path, nano::node_config const &, nano::work_pool &, nano::node_flags = nano::node_flags (), unsigned seq = 0);
	~node ();

public:
	void background (std::function<void ()> action_a);
	bool copy_with_compaction (std::filesystem::path const &);
	void keepalive (std::string const &, uint16_t);
	void start ();
	void stop ();
	bool is_stopped () const;
	std::shared_ptr<nano::node> shared ();
	int store_version ();
	void process_confirmed (nano::election_status const &, uint64_t = 0);
	void process_active (std::shared_ptr<nano::block> const &);
	std::optional<nano::block_status> process_local (std::shared_ptr<nano::block> const &);
	void process_local_async (std::shared_ptr<nano::block> const &);
	void keepalive_preconfigured ();
	std::shared_ptr<nano::block> block (nano::block_hash const &);
	std::pair<nano::uint128_t, nano::uint128_t> balance_pending (nano::account const &, bool only_confirmed);
	nano::uint128_t weight (nano::account const &);
	nano::uint128_t minimum_principal_weight ();
	void ongoing_bootstrap ();
	void ongoing_peer_store ();
	void backup_wallet ();
	void search_receivable_all ();
	void bootstrap_wallet ();
	bool collect_ledger_pruning_targets (std::deque<nano::block_hash> &, nano::account &, uint64_t const, uint64_t const, uint64_t const);
	void ledger_pruning (uint64_t const, bool);
	void ongoing_ledger_pruning ();
	int price (nano::uint128_t const &, int);
	// The default difficulty updates to base only when the first epoch_2 block is processed
	uint64_t default_difficulty (nano::work_version const) const;
	uint64_t default_receive_difficulty (nano::work_version const) const;
	uint64_t max_work_generate_difficulty (nano::work_version const) const;
	bool local_work_generation_enabled () const;
	bool work_generation_enabled () const;
	std::optional<uint64_t> work_generate_blocking (nano::block &, uint64_t);
	std::optional<uint64_t> work_generate_blocking (nano::work_version const, nano::root const &, uint64_t, std::optional<nano::account> const & = std::nullopt);
	void work_generate (nano::work_version const, nano::root const &, uint64_t, std::function<void (std::optional<uint64_t>)>, std::optional<nano::account> const & = std::nullopt, bool const = false);
	void add_initial_peers ();
	void start_election (std::shared_ptr<nano::block> const & block);
	bool block_confirmed (nano::block_hash const &);
	bool block_confirmed_or_being_confirmed (nano::store::transaction const &, nano::block_hash const &);
	bool block_confirmed_or_being_confirmed (nano::block_hash const &);
	void do_rpc_callback (boost::asio::ip::tcp::resolver::iterator i_a, std::string const &, uint16_t, std::shared_ptr<std::string> const &, std::shared_ptr<std::string> const &, std::shared_ptr<boost::asio::ip::tcp::resolver> const &);
	void ongoing_online_weight_calculation ();
	void ongoing_online_weight_calculation_queue ();
	bool online () const;
	bool init_error () const;
	std::pair<uint64_t, std::unordered_map<nano::account, nano::uint128_t>> get_bootstrap_weights () const;
	uint64_t get_confirmation_height (store::transaction const &, nano::account &);
	/*
	 * Attempts to bootstrap block. This is the best effort, there is no guarantee that the block will be bootstrapped.
	 */
	void bootstrap_block (nano::block_hash const &);
	nano::account get_node_id () const;
	nano::telemetry_data local_telemetry () const;

public:
	nano::keypair node_id; // ported
	rsnano::async_runtime & async_rt; // ported
	boost::asio::io_context & io_ctx; // ported
	boost::latch node_initialized_latch;
	std::shared_ptr<nano::node_observers> observers;
	std::shared_ptr<nano::node_config> config; // ported
	nano::network_params network_params; // ported
	std::shared_ptr<nano::logger> logger; // ported
	std::shared_ptr<nano::stats> stats; // ported
	std::shared_ptr<nano::thread_pool> workers; // ported
	std::shared_ptr<nano::thread_pool> bootstrap_workers; // ported
	nano::node_flags flags; // ported
	nano::work_pool & work; // ported
	nano::distributed_work_factory distributed_work;
	std::unique_ptr<nano::store::component> store_impl; // ported
	nano::store::component & store; // ported
	nano::unchecked_map unchecked; // ported
	std::unique_ptr<nano::wallets_store> wallets_store_impl; // ported
	nano::wallets_store & wallets_store; // ported
	nano::ledger ledger; // ported
	nano::outbound_bandwidth_limiter outbound_limiter; // ported
	std::shared_ptr<nano::network> network;
	std::shared_ptr<nano::telemetry> telemetry; // ported
	nano::bootstrap_server bootstrap_server; // ported
	std::filesystem::path application_path;
	nano::port_mapping port_mapping;
	nano::online_reps online_reps; // ported
	nano::representative_register representative_register; // ported
	nano::rep_tiers rep_tiers; // ported
	nano::vote_processor_queue vote_processor_queue; // ported
	unsigned warmed_up;
	nano::local_vote_history history; // ported
	nano::confirming_set confirming_set; // ported
	nano::vote_cache vote_cache; // ported
	nano::block_processor block_processor; // ported
	nano::wallets wallets; // mostly ported
	nano::vote_generator generator; // ported
	nano::vote_generator final_generator; // ported
	nano::active_transactions active; // ported
	nano::vote_processor vote_processor; // ported
	nano::websocket_server websocket; // ported
	nano::bootstrap_initiator bootstrap_initiator; // ported
	nano::rep_crawler rep_crawler; // ported
	std::shared_ptr<nano::transport::tcp_listener> tcp_listener; // ported

private: // Placed here to maintain initialization order
	std::unique_ptr<nano::scheduler::component> scheduler_impl; // ported

public:
	nano::scheduler::component & scheduler; // ported
	nano::request_aggregator aggregator; // ported
	nano::backlog_population backlog; // ported
	nano::bootstrap_ascending::service ascendboot; // ported
	nano::local_block_broadcaster local_block_broadcaster; // ported
	nano::process_live_dispatcher process_live_dispatcher; // ported
	nano::live_message_processor live_message_processor; // ported

	std::chrono::steady_clock::time_point const startup_time;
	std::chrono::seconds unchecked_cutoff = std::chrono::seconds (7 * 24 * 60 * 60); // Week
	std::atomic<bool> unresponsive_work_peers{ false };
	std::atomic<bool> stopped{ false };
	static double constexpr price_max = 16.0;
	static double constexpr free_cutoff = 1024.0;
	// For tests only
	unsigned node_seq;
	// For tests only
	std::optional<uint64_t> work_generate_blocking (nano::block &);
	// For tests only
	std::optional<uint64_t> work_generate_blocking (nano::root const &, uint64_t);
	// For tests only
	std::optional<uint64_t> work_generate_blocking (nano::root const &);

public: // Testing convenience functions
	/**
		Creates a new write transaction and inserts `block' and returns result
		Transaction is comitted before function return
	 */
	[[nodiscard]] nano::block_status process (std::shared_ptr<nano::block> block);
	[[nodiscard]] nano::block_status process (store::write_transaction const &, std::shared_ptr<nano::block> block);
	nano::block_hash latest (nano::account const &);
	nano::uint128_t balance (nano::account const &);

private:
	void long_inactivity_cleanup ();

	static std::string make_logger_identifier (nano::keypair const & node_id);
};

nano::keypair load_or_create_node_id (std::filesystem::path const & application_path);
std::unique_ptr<container_info_component> collect_container_info (node & node, std::string const & name);

nano::node_flags const & inactive_node_flag_defaults ();

}
