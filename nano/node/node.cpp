#include "nano/lib/numbers.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/common.hpp>
#include <nano/node/daemonconfig.hpp>
#include <nano/node/election_status.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/make_store.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/hinted.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/optimistic.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/node/telemetry.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>
#include <nano/store/write_queue.hpp>

#include <boost/property_tree/json_parser.hpp>

#include <algorithm>
#include <iterator>
#include <memory>

double constexpr nano::node::price_max;
double constexpr nano::node::free_cutoff;

/*
 * configs
 */

nano::backlog_population::config nano::backlog_population_config (const nano::node_config & config)
{
	nano::backlog_population::config cfg{};
	cfg.enabled = config.frontiers_confirmation != nano::frontiers_confirmation_mode::disabled;
	cfg.frequency = config.backlog_scan_frequency;
	cfg.batch_size = config.backlog_scan_batch_size;
	return cfg;
}

/*
 * node
 */

void nano::node::keepalive (std::string const & address_a, uint16_t port_a)
{
	rsnano::rsn_rep_crawler_keepalive (rep_crawler.handle, address_a.c_str (), port_a);
}

namespace
{
nano::keypair get_node_id_key_pair (rsnano::NodeHandle const * handle)
{
	nano::raw_key prv;
	rsnano::rsn_node_node_id (handle, prv.bytes.data ());
	return std::move (nano::keypair{ std::move (prv) });
}

std::shared_ptr<nano::node_config> get_node_config (rsnano::NodeHandle const * handle)
{
	rsnano::NodeConfigDto dto;
	rsn_node_config (handle, &dto);
	auto config = std::make_shared<nano::node_config> ();
	config->load_dto (dto);
	return config;
}

namespace
{
	void delete_observers_context (void * context)
	{
		auto observers = static_cast<std::weak_ptr<nano::node_observers> *> (context);
		delete observers;
	}

	void call_election_ended (void * context, rsnano::ElectionStatusHandle * status_handle,
	rsnano::VoteWithWeightInfoVecHandle * votes_handle, uint8_t const * account_bytes,
	uint8_t const * amount_bytes, bool is_state_send, bool is_state_epoch)
	{
		auto observers = static_cast<std::weak_ptr<nano::node_observers> *> (context);
		auto obs = observers->lock ();
		if (!obs)
		{
			return;
		}

		nano::election_status status{ status_handle };

		std::vector<nano::vote_with_weight_info> votes;
		auto len = rsnano::rsn_vote_with_weight_info_vec_len (votes_handle);
		for (auto i = 0; i < len; ++i)
		{
			rsnano::VoteWithWeightInfoDto dto;
			rsnano::rsn_vote_with_weight_info_vec_get (votes_handle, i, &dto);
			votes.emplace_back (dto);
		}
		rsnano::rsn_vote_with_weight_info_vec_destroy (votes_handle);

		auto account{ nano::account::from_bytes (account_bytes) };
		auto amount{ nano::amount::from_bytes (amount_bytes) };

		obs->blocks.notify (status, votes, account, amount.number (), is_state_send, is_state_epoch);
	}

	void call_account_balance_changed (void * context, uint8_t const * account, bool is_pending)
	{
		auto observers = static_cast<std::weak_ptr<nano::node_observers> *> (context);
		auto obs = observers->lock ();
		if (!obs)
		{
			return;
		}
		obs->account_balance.notify (nano::account::from_bytes (account), is_pending);
	}

	void on_vote_processed (void * context, rsnano::VoteHandle * vote_handle, rsnano::ChannelHandle * channel_handle, uint8_t code)
	{
		auto observers = static_cast<std::weak_ptr<nano::node_observers> *> (context);
		auto obs = observers->lock ();
		if (!obs)
		{
			return;
		}
		auto vote = std::make_shared<nano::vote> (vote_handle);
		auto channel = nano::transport::channel_handle_to_channel (channel_handle);
		obs->vote.notify (vote, channel, static_cast<nano::vote_code> (code));
	}

}

rsnano::NodeHandle * create_node_handle (
rsnano::async_runtime & async_rt_a,
std::filesystem::path const & application_path_a,
nano::node_config const & config_a,
nano::work_pool & work_a,
nano::node_flags flags_a,
unsigned seq,
std::shared_ptr<nano::node_observers> & observers)
{
	auto config_dto{ config_a.to_dto () };
	auto params_dto{ config_a.network_params.to_dto () };
	auto socket_observer = new std::weak_ptr<nano::node_observers> (observers);
	auto observers_context = new std::weak_ptr<nano::node_observers> (observers);
	return rsnano::rsn_node_create (application_path_a.c_str (), async_rt_a.handle, &config_dto, &params_dto,
	flags_a.handle, work_a.handle, socket_observer, observers_context, delete_observers_context,
	call_election_ended, call_account_balance_changed, on_vote_processed);
}
}

nano::node::node (rsnano::async_runtime & async_rt, uint16_t peering_port_a, std::filesystem::path const & application_path_a, nano::work_pool & work_a, nano::node_flags flags_a, unsigned seq) :
	node (async_rt, application_path_a, nano::node_config (peering_port_a), work_a, flags_a, seq)
{
}

nano::node::node (rsnano::async_runtime & async_rt_a, std::filesystem::path const & application_path_a, nano::node_config const & config_a, nano::work_pool & work_a, nano::node_flags flags_a, unsigned seq) :
	observers{ std::make_shared<nano::node_observers> () },
	handle{ create_node_handle (async_rt_a, application_path_a, config_a, work_a, flags_a, seq, observers) },
	node_id{ get_node_id_key_pair (handle) },
	async_rt{ async_rt_a },
	io_ctx (async_rt_a.io_ctx),
	config{ get_node_config (handle) },
	network_params{ config_a.network_params },
	logger{ std::make_shared<nano::logger> (make_logger_identifier (node_id)) },
	stats{ std::make_shared<nano::stats> (rsnano::rsn_node_stats (handle)) },
	workers{ std::make_shared<nano::thread_pool> (rsnano::rsn_node_workers (handle)) },
	bootstrap_workers{ std::make_shared<nano::thread_pool> (rsnano::rsn_node_bootstrap_workers (handle)) },
	flags (flags_a),
	work (work_a),
	distributed_work (rsnano::rsn_node_distributed_work (handle)),
	store (rsnano::rsn_node_store (handle)),
	unchecked{ rsn_node_unchecked (handle) },
	ledger (rsnano::rsn_node_ledger (handle), store, network_params.ledger),
	outbound_limiter{ rsnano::rsn_node_outbound_bandwidth_limiter (handle) },
	// empty `config.peering_port` means the user made no port choice at all;
	// otherwise, any value is considered, with `0` having the special meaning of 'let the OS pick a port instead'
	network{ std::make_shared<nano::network> (*this, config_a.peering_port.value_or (0), rsnano::rsn_node_syn_cookies (handle), rsnano::rsn_node_tcp_channels (handle), rsnano::rsn_node_tcp_message_manager (handle), rsnano::rsn_node_network_filter (handle)) },
	telemetry (std::make_shared<nano::telemetry> (rsnano::rsn_node_telemetry (handle))),
	bootstrap_initiator (rsnano::rsn_node_bootstrap_initiator (handle)),
	bootstrap_server{ rsnano::rsn_node_bootstrap_server (handle) },
	// BEWARE: `bootstrap` takes `network.port` instead of `config.peering_port` because when the user doesn't specify
	//         a peering port and wants the OS to pick one, the picking happens when `network` gets initialized
	//         (if UDP is active, otherwise it happens when `bootstrap` gets initialized), so then for TCP traffic
	//         we want to tell `bootstrap` to use the already picked port instead of itself picking a different one.
	//         Thus, be very careful if you change the order: if `bootstrap` gets constructed before `network`,
	//         the latter would inherit the port from the former (if TCP is active, otherwise `network` picks first)
	//
	tcp_listener{ std::make_shared<nano::transport::tcp_listener> (rsnano::rsn_node_tcp_listener (handle)) },
	application_path (application_path_a),
	representative_register (rsnano::rsn_node_representative_register (handle)),
	rep_crawler (rsnano::rsn_node_rep_crawler (handle), *this),
	rep_tiers{ rsnano::rsn_node_rep_tiers (handle) },
	vote_processor_queue{
		rsnano::rsn_node_vote_processor_queue (handle)
	},
	vote_processor (rsnano::rsn_node_vote_processor (handle)),
	warmed_up (0),
	block_processor (rsnano::rsn_node_block_processor (handle)),
	online_reps (rsnano::rsn_node_online_reps (handle)),
	history{ rsnano::rsn_node_history (handle) },
	confirming_set (rsnano::rsn_node_confirming_set (handle)),
	vote_cache{ rsnano::rsn_node_vote_cache (handle) },
	wallets{ rsnano::rsn_node_wallets (handle) },
	generator{ rsnano::rsn_node_vote_generator (handle) },
	final_generator{ rsnano::rsn_node_final_generator (handle) },
	active (*this, rsnano::rsn_node_active (handle)),
	scheduler_impl{ std::make_unique<nano::scheduler::component> (handle) },
	scheduler{ *scheduler_impl },
	aggregator (rsnano::rsn_node_request_aggregator (handle)),
	backlog{ rsnano::rsn_node_backlog_population (handle) },
	ascendboot{ rsnano::rsn_node_ascendboot (handle) },
	websocket{ rsnano::rsn_node_websocket (handle) },
	local_block_broadcaster{ rsnano::rsn_node_local_block_broadcaster (handle) },
	process_live_dispatcher{ rsnano::rsn_node_process_live_dispatcher (handle) },
	startup_time (std::chrono::steady_clock::now ()),
	node_seq (seq),
	live_message_processor{ rsnano::rsn_node_live_message_processor (handle) },
	network_threads{ rsnano::rsn_node_network_threads (handle) }
{
}

nano::node::~node ()
{
	logger->debug (nano::log::type::node, "Destructing node...");
	stop ();
	rsnano::rsn_node_destroy (handle);
}

namespace
{
void call_post_callback (void * callback_handle)
{
	auto callback = static_cast<std::function<void ()> *> (callback_handle);
	(*callback) ();
}

void delete_post_callback (void * callback_handle)
{
	auto callback = static_cast<std::function<void ()> *> (callback_handle);
	delete callback;
}
}

bool nano::node::copy_with_compaction (std::filesystem::path const & destination)
{
	return store.copy_db (destination);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (node & node, std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_node_collect_container_info (node.handle, name.c_str ()));
}

void nano::node::process_active (std::shared_ptr<nano::block> const & incoming)
{
	block_processor.process_active (incoming);
}

[[nodiscard]] nano::block_status nano::node::process (store::write_transaction const & transaction, std::shared_ptr<nano::block> block)
{
	return ledger.process (transaction, block);
}

nano::block_status nano::node::process (std::shared_ptr<nano::block> block)
{
	auto const transaction = store.tx_begin_write ({ tables::accounts, tables::blocks, tables::pending, tables::rep_weights });
	return process (*transaction, block);
}

std::optional<nano::block_status> nano::node::process_local (std::shared_ptr<nano::block> const & block_a)
{
	return block_processor.add_blocking (block_a, nano::block_source::local);
}

void nano::node::process_local_async (std::shared_ptr<nano::block> const & block_a)
{
	block_processor.add (block_a, nano::block_source::local);
}

void nano::node::start ()
{
	rsnano::rsn_node_start (handle);
	if (flags.enable_pruning ())
	{
		auto this_l (shared ());
		workers->push_task ([this_l] () {
			this_l->ongoing_ledger_pruning ();
		});
	}
	if (!flags.disable_rep_crawler ())
	{
		rep_crawler.start ();
	}
	ongoing_peer_store ();
	ongoing_online_weight_calculation_queue ();

	bool tcp_enabled (false);
	if (config->tcp_incoming_connections_max > 0 && !(flags.disable_bootstrap_listener () && flags.disable_tcp_realtime ()))
	{
		auto listener_w{ tcp_listener->weak_from_this () };
		tcp_listener->start ([listener_w] (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const & ec_a) {
			auto listener_l{ listener_w.lock () };
			if (!listener_l)
			{
				return false;
			}
			if (!ec_a)
			{
				listener_l->accept_action (ec_a, new_connection);
			}
			return true;
		});
		tcp_enabled = true;

		if (network->get_port () != tcp_listener->endpoint ().port ())
		{
			network->set_port (tcp_listener->endpoint ().port ());
		}

		logger->info (nano::log::type::node, "Node peering port: {}", network->port.load ());
	}

	if (!flags.disable_backup ())
	{
		backup_wallet ();
	}
	if (!flags.disable_search_pending ())
	{
		search_receivable_all ();
	}
	if (!flags.disable_wallet_bootstrap ())
	{
		// Delay to start wallet lazy bootstrap
		auto this_l (shared ());
		workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::minutes (1), [this_l] () {
			this_l->bootstrap_wallet ();
		});
	}
	unchecked.start ();
	wallets.start_actions ();
	rep_tiers.start ();
	vote_processor.start ();
	block_processor.start ();
	active.start ();
	generator.start ();
	final_generator.start ();
	confirming_set.start ();
	scheduler.start ();
	backlog.start ();
	bootstrap_server.start ();
	if (!flags.disable_ascending_bootstrap ())
	{
		ascendboot.start ();
	}
	websocket.start ();
	telemetry->start ();
	local_block_broadcaster.start ();
}

void nano::node::stop ()
{
	// Ensure stop can only be called once
	if (stopped.exchange (true))
	{
		return;
	}

	logger->info (nano::log::type::node, "Node stopping...");

	// Cancels ongoing work generation tasks, which may be blocking other threads
	// No tasks may wait for work generation in I/O threads, or termination signal capturing will be unable to call node::stop()
	distributed_work.stop ();
	backlog.stop ();
	if (!flags.disable_ascending_bootstrap ())
	{
		ascendboot.stop ();
	}
	rep_crawler.stop ();
	unchecked.stop ();
	block_processor.stop ();
	aggregator.stop ();
	vote_processor.stop ();
	rep_tiers.stop ();
	scheduler.stop ();
	active.stop ();
	generator.stop ();
	final_generator.stop ();
	confirming_set.stop ();
	telemetry->stop ();
	websocket.stop ();
	bootstrap_server.stop ();
	bootstrap_initiator.stop ();
	tcp_listener->stop ();
	wallets.stop_actions ();
	stats->stop ();
	workers->stop ();
	local_block_broadcaster.stop ();
	network_threads.stop (); // Stop network last to avoid killing in-use sockets
							 //
	// work pool is not stopped on purpose due to testing setup
}

bool nano::node::is_stopped () const
{
	return stopped;
}

void nano::node::keepalive_preconfigured ()
{
	for (auto const & peer : config->preconfigured_peers)
	{
		// can't use `network.port` here because preconfigured peers are referenced
		// just by their address, so we rely on them listening on the default port
		//
		keepalive (peer, network_params.network.default_node_port);
	}
}

nano::block_hash nano::node::latest (nano::account const & account_a)
{
	auto const transaction (store.tx_begin_read ());
	return ledger.latest (*transaction, account_a);
}

nano::uint128_t nano::node::balance (nano::account const & account_a)
{
	auto const transaction (store.tx_begin_read ());
	return ledger.account_balance (*transaction, account_a);
}

std::shared_ptr<nano::block> nano::node::block (nano::block_hash const & hash_a)
{
	auto const transaction (store.tx_begin_read ());
	return ledger.block (*transaction, hash_a);
}

std::pair<nano::uint128_t, nano::uint128_t> nano::node::balance_pending (nano::account const & account_a, bool only_confirmed_a)
{
	std::pair<nano::uint128_t, nano::uint128_t> result;
	auto const transaction (store.tx_begin_read ());
	result.first = ledger.account_balance (*transaction, account_a, only_confirmed_a);
	result.second = ledger.account_receivable (*transaction, account_a, only_confirmed_a);
	return result;
}

nano::uint128_t nano::node::weight (nano::account const & account_a)
{
	auto txn{ ledger.store.tx_begin_read () };
	return ledger.weight_exact (*txn, account_a);
}

nano::uint128_t nano::node::minimum_principal_weight ()
{
	return online_reps.minimum_principal_weight ();
}

void nano::node::ongoing_peer_store ()
{
	auto endpoints{ network->tcp_channels->get_peers () };
	bool stored (false);
	if (!endpoints.empty ())
	{
		// Clear all peers then refresh with the current list of peers
		auto transaction (store.tx_begin_write ({ tables::peers }));
		store.peer ().clear (*transaction);
		for (auto const & endpoint : endpoints)
		{
			store.peer ().put (*transaction, nano::endpoint_key{ endpoint.address ().to_v6 ().to_bytes (), endpoint.port () });
		}
		stored = true;
	}

	std::weak_ptr<nano::node> node_w (shared_from_this ());
	workers->add_timed_task (std::chrono::steady_clock::now () + network_params.network.peer_dump_interval, [node_w] () {
		if (auto node_l = node_w.lock ())
		{
			node_l->ongoing_peer_store ();
		}
	});
}

void nano::node::backup_wallet ()
{
	auto backup_path (application_path / "backup");
	wallets.backup (backup_path);
	auto this_l (shared ());
	workers->add_timed_task (std::chrono::steady_clock::now () + network_params.node.backup_interval, [this_l] () {
		this_l->backup_wallet ();
	});
}

void nano::node::search_receivable_all ()
{
	// Reload wallets from disk
	wallets.reload ();
	// Search pending
	wallets.search_receivable_all ();
	auto this_l (shared ());
	workers->add_timed_task (std::chrono::steady_clock::now () + network_params.node.search_pending_interval, [this_l] () {
		this_l->search_receivable_all ();
	});
}

void nano::node::bootstrap_wallet ()
{
	std::deque<nano::account> accounts;
	auto accs{ wallets.get_accounts (128) };
	std::copy (accs.begin (), accs.end (), std::back_inserter (accounts));
	if (!accounts.empty ())
	{
		bootstrap_initiator.bootstrap_wallet (accounts);
	}
}

bool nano::node::collect_ledger_pruning_targets (std::deque<nano::block_hash> & pruning_targets_a, nano::account & last_account_a, uint64_t const batch_read_size_a, uint64_t const max_depth_a, uint64_t const cutoff_time_a)
{
	uint64_t read_operations (0);
	bool finish_transaction (false);
	auto const transaction (store.tx_begin_read ());
	for (auto i (store.confirmation_height ().begin (*transaction, last_account_a)), n (store.confirmation_height ().end ()); i != n && !finish_transaction;)
	{
		++read_operations;
		auto const & account (i->first);
		nano::block_hash hash (i->second.frontier ());
		uint64_t depth (0);
		while (!hash.is_zero () && depth < max_depth_a)
		{
			auto block (ledger.block (*transaction, hash));
			if (block != nullptr)
			{
				if (block->sideband ().timestamp () > cutoff_time_a || depth == 0)
				{
					hash = block->previous ();
				}
				else
				{
					break;
				}
			}
			else
			{
				release_assert (depth != 0);
				hash = 0;
			}
			if (++depth % batch_read_size_a == 0)
			{
				transaction->refresh ();
			}
		}
		if (!hash.is_zero ())
		{
			pruning_targets_a.push_back (hash);
		}
		read_operations += depth;
		if (read_operations >= batch_read_size_a)
		{
			last_account_a = account.number () + 1;
			finish_transaction = true;
		}
		else
		{
			++i;
		}
	}
	return !finish_transaction || last_account_a.is_zero ();
}

void nano::node::ledger_pruning (uint64_t const batch_size_a, bool bootstrap_weight_reached_a)
{
	uint64_t const max_depth (config->max_pruning_depth != 0 ? config->max_pruning_depth : std::numeric_limits<uint64_t>::max ());
	uint64_t const cutoff_time (bootstrap_weight_reached_a ? nano::seconds_since_epoch () - config->max_pruning_age.count () : std::numeric_limits<uint64_t>::max ());
	uint64_t pruned_count (0);
	uint64_t transaction_write_count (0);
	nano::account last_account (1); // 0 Burn account is never opened. So it can be used to break loop
	std::deque<nano::block_hash> pruning_targets;
	bool target_finished (false);
	while ((transaction_write_count != 0 || !target_finished) && !stopped)
	{
		// Search pruning targets
		while (pruning_targets.size () < batch_size_a && !target_finished && !stopped)
		{
			target_finished = collect_ledger_pruning_targets (pruning_targets, last_account, batch_size_a * 2, max_depth, cutoff_time);
		}
		// Pruning write operation
		transaction_write_count = 0;
		if (!pruning_targets.empty () && !stopped)
		{
			auto scoped_write_guard = ledger.wait (nano::store::writer::pruning);
			auto write_transaction (store.tx_begin_write ({ tables::blocks, tables::pruned }));
			while (!pruning_targets.empty () && transaction_write_count < batch_size_a && !stopped)
			{
				auto const & pruning_hash (pruning_targets.front ());
				auto account_pruned_count (ledger.pruning_action (*write_transaction, pruning_hash, batch_size_a));
				transaction_write_count += account_pruned_count;
				pruning_targets.pop_front ();
			}
			pruned_count += transaction_write_count;

			logger->debug (nano::log::type::prunning, "Pruned blocks: {}", pruned_count);
		}
	}

	logger->debug (nano::log::type::prunning, "Total recently pruned block count: {}", pruned_count);
}

void nano::node::ongoing_ledger_pruning ()
{
	auto bootstrap_weight_reached (ledger.block_count () >= ledger.get_bootstrap_weight_max_blocks ());
	ledger_pruning (flags.block_processor_batch_size () != 0 ? flags.block_processor_batch_size () : 2 * 1024, bootstrap_weight_reached);
	auto const ledger_pruning_interval (bootstrap_weight_reached ? config->max_pruning_age : std::min (config->max_pruning_age, std::chrono::seconds (15 * 60)));
	auto this_l (shared ());
	workers->add_timed_task (std::chrono::steady_clock::now () + ledger_pruning_interval, [this_l] () {
		this_l->workers->push_task ([this_l] () {
			this_l->ongoing_ledger_pruning ();
		});
	});
}

int nano::node::price (nano::uint128_t const & balance_a, int amount_a)
{
	debug_assert (balance_a >= amount_a * nano::Gxrb_ratio);
	auto balance_l (balance_a);
	double result (0.0);
	for (auto i (0); i < amount_a; ++i)
	{
		balance_l -= nano::Gxrb_ratio;
		auto balance_scaled ((balance_l / nano::Mxrb_ratio).convert_to<double> ());
		auto units (balance_scaled / 1000.0);
		auto unit_price (((free_cutoff - units) / free_cutoff) * price_max);
		result += std::min (std::max (0.0, unit_price), price_max);
	}
	return static_cast<int> (result * 100.0);
}

uint64_t nano::node::default_difficulty (nano::work_version const version_a) const
{
	uint64_t result{ std::numeric_limits<uint64_t>::max () };
	switch (version_a)
	{
		case nano::work_version::work_1:
			result = network_params.work.threshold_base (version_a);
			break;
		default:
			debug_assert (false && "Invalid version specified to default_difficulty");
	}
	return result;
}

uint64_t nano::node::default_receive_difficulty (nano::work_version const version_a) const
{
	uint64_t result{ std::numeric_limits<uint64_t>::max () };
	switch (version_a)
	{
		case nano::work_version::work_1:
			result = network_params.work.get_epoch_2_receive ();
			break;
		default:
			debug_assert (false && "Invalid version specified to default_receive_difficulty");
	}
	return result;
}

uint64_t nano::node::max_work_generate_difficulty (nano::work_version const version_a) const
{
	return nano::difficulty::from_multiplier (config->max_work_generate_multiplier, default_difficulty (version_a));
}

bool nano::node::local_work_generation_enabled () const
{
	return work.work_generation_enabled ();
}

bool nano::node::work_generation_enabled () const
{
	return distributed_work.work_generation_enabled ();
}

std::optional<uint64_t> nano::node::work_generate_blocking (nano::block & block_a, uint64_t difficulty_a)
{
	return distributed_work.make_blocking (block_a, difficulty_a);
}

void nano::node::work_generate (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> callback_a, std::optional<nano::account> const & account_a, bool secondary_work_peers_a)
{
	distributed_work.make (version_a, root_a, difficulty_a, callback_a, account_a, secondary_work_peers_a);
}

std::optional<uint64_t> nano::node::work_generate_blocking (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::optional<nano::account> const & account_a)
{
	return distributed_work.make_blocking (version_a, root_a, difficulty_a, account_a);
}

std::optional<uint64_t> nano::node::work_generate_blocking (nano::block & block_a)
{
	debug_assert (network_params.network.is_dev_network ());
	return work_generate_blocking (block_a, default_difficulty (nano::work_version::work_1));
}

std::optional<uint64_t> nano::node::work_generate_blocking (nano::root const & root_a)
{
	debug_assert (network_params.network.is_dev_network ());
	return work_generate_blocking (root_a, default_difficulty (nano::work_version::work_1));
}

std::optional<uint64_t> nano::node::work_generate_blocking (nano::root const & root_a, uint64_t difficulty_a)
{
	debug_assert (network_params.network.is_dev_network ());
	return work_generate_blocking (nano::work_version::work_1, root_a, difficulty_a);
}

void nano::node::add_initial_peers ()
{
	if (flags.disable_add_initial_peers ())
	{
		logger->warn (nano::log::type::node, "Not adding initial peers because `disable_add_initial_peers` flag is set");
		return;
	}

	std::vector<nano::endpoint> initial_peers;
	{
		auto transaction = store.tx_begin_read ();
		for (auto i (store.peer ().begin (*transaction)), n (store.peer ().end ()); i != n; ++i)
		{
			nano::endpoint endpoint (boost::asio::ip::address_v6 (i->first.address_bytes ()), i->first.port ());
			initial_peers.push_back (endpoint);
		}
	}

	logger->info (nano::log::type::node, "Adding cached initial peers: {}", initial_peers.size ());

	for (auto const & peer : initial_peers)
	{
		network->merge_peer (peer);
	}
}

void nano::node::start_election (std::shared_ptr<nano::block> const & block)
{
	scheduler.manual.push (block);
}

bool nano::node::block_confirmed (nano::block_hash const & hash_a)
{
	auto transaction (store.tx_begin_read ());
	return ledger.block_confirmed (*transaction, hash_a);
}

bool nano::node::block_confirmed_or_being_confirmed (nano::store::transaction const & transaction, nano::block_hash const & hash_a)
{
	return confirming_set.exists (hash_a) || ledger.block_confirmed (transaction, hash_a);
}

bool nano::node::block_confirmed_or_being_confirmed (nano::block_hash const & hash_a)
{
	return block_confirmed_or_being_confirmed (*store.tx_begin_read (), hash_a);
}

void nano::node::ongoing_online_weight_calculation_queue ()
{
	std::weak_ptr<nano::node> node_w (shared_from_this ());
	workers->add_timed_task (std::chrono::steady_clock::now () + (std::chrono::seconds (network_params.node.weight_period)), [node_w] () {
		if (auto node_l = node_w.lock ())
		{
			node_l->ongoing_online_weight_calculation ();
		}
	});
}

bool nano::node::online () const
{
	return representative_register.total_weight () > online_reps.delta ();
}

void nano::node::ongoing_online_weight_calculation ()
{
	online_reps.sample ();
	ongoing_online_weight_calculation_queue ();
}

void nano::node::process_confirmed (nano::election_status const & status_a, uint64_t iteration_a)
{
	active.process_confirmed (status_a, iteration_a);
}

std::shared_ptr<nano::node> nano::node::shared ()
{
	return shared_from_this ();
}

int nano::node::store_version ()
{
	auto transaction (store.tx_begin_read ());
	return store.version ().get (*transaction);
}

bool nano::node::init_error () const
{
	return store.init_error ();
}

void nano::node::bootstrap_block (const nano::block_hash & hash)
{
	// If we are running pruning node check if block was not already pruned
	if (!ledger.pruning_enabled () || !store.pruned ().exists (*store.tx_begin_read (), hash))
	{
		// We don't have the block, try to bootstrap it
		// TODO: Use ascending bootstraper to bootstrap block hash
	}
}

/** Convenience function to easily return the confirmation height of an account. */
uint64_t nano::node::get_confirmation_height (store::transaction const & transaction_a, nano::account & account_a)
{
	nano::confirmation_height_info info;
	store.confirmation_height ().get (transaction_a, account_a, info);
	return info.height ();
}

nano::account nano::node::get_node_id () const
{
	return node_id.pub;
};

nano::telemetry_data nano::node::local_telemetry () const
{
	return telemetry->local_telemetry ();
}

std::string nano::node::make_logger_identifier (const nano::keypair & node_id)
{
	// Node identifier consists of first 10 characters of node id
	return node_id.pub.to_node_id ().substr (0, 10);
}
