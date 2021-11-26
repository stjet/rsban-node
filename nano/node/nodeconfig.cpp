#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/rpcconfig.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/transport/transport.hpp>

#include <crypto/cryptopp/words.h>

#include <boost/format.hpp>

namespace
{
char const * preconfigured_peers_key = "preconfigured_peers";
char const * signature_checker_threads_key = "signature_checker_threads";
char const * pow_sleep_interval_key = "pow_sleep_interval";
std::string const default_test_peer_network = nano::get_env_or_default ("NANO_TEST_PEER_NETWORK", "peering-test.nano.org");
}

nano::node_config::node_config (nano::network_params & network_params) :
	node_config (0, nano::logging (), network_params)
{
}

nano::node_config::node_config (uint16_t peering_port_a, nano::logging const & logging_a, nano::network_params & network_params) :
	network_params{ network_params },
	logging{ logging_a },
	ipc_config{ network_params.network },
	websocket_config{ network_params.network }
{
	rsnano::NodeConfigDto dto;
	auto network_params_dto{ network_params.to_dto () };
	auto logging_dto{ logging.to_dto () };
	rsnano::rsn_node_config_create (&dto, peering_port_a, &logging_dto, &network_params_dto);
	peering_port = dto.peering_port;
	bootstrap_fraction_numerator = dto.bootstrap_fraction_numerator;
	std::copy (std::begin (dto.receive_minimum), std::end (dto.receive_minimum), std::begin (receive_minimum.bytes));
	std::copy (std::begin (dto.online_weight_minimum), std::end (dto.online_weight_minimum), std::begin (online_weight_minimum.bytes));
	election_hint_weight_percent = dto.election_hint_weight_percent;
	password_fanout = dto.password_fanout;
	io_threads = dto.io_threads;
	network_threads = dto.network_threads;
	work_threads = dto.work_threads;
	signature_checker_threads = dto.signature_checker_threads;
	enable_voting = dto.enable_voting;
	bootstrap_connections = dto.bootstrap_connections;
	bootstrap_connections_max = dto.bootstrap_connections_max;
	bootstrap_initiator_threads = dto.bootstrap_initiator_threads;
	bootstrap_frontier_request_count = dto.bootstrap_frontier_request_count;
	block_processor_batch_max_time = std::chrono::milliseconds (dto.block_processor_batch_max_time_ms);
	allow_local_peers = dto.allow_local_peers;
	std::copy (std::begin (dto.vote_minimum), std::end (dto.vote_minimum), std::begin (vote_minimum.bytes));
	vote_generator_delay = std::chrono::milliseconds (dto.vote_generator_delay_ms);
	vote_generator_threshold = dto.vote_generator_threshold;
	unchecked_cutoff_time = std::chrono::seconds (dto.unchecked_cutoff_time_s);
	tcp_io_timeout = std::chrono::seconds (dto.tcp_io_timeout_s);
	pow_sleep_interval = std::chrono::nanoseconds (dto.pow_sleep_interval_ns);
	external_address = std::string (reinterpret_cast<const char *> (dto.external_address), dto.external_address_len);
	external_port = dto.external_port;
	tcp_incoming_connections_max = dto.tcp_incoming_connections_max;
	use_memory_pools = dto.use_memory_pools;
	confirmation_history_size = dto.confirmation_history_size;
	active_elections_size = dto.active_elections_size;
	bandwidth_limit = dto.bandwidth_limit;
	bandwidth_limit_burst_ratio = dto.bandwidth_limit_burst_ratio;
	conf_height_processor_batch_min_time = std::chrono::milliseconds (dto.conf_height_processor_batch_min_time_ms);
	backup_before_upgrade = dto.backup_before_upgrade;
	max_work_generate_multiplier = dto.max_work_generate_multiplier;
	frontiers_confirmation = static_cast<nano::frontiers_confirmation_mode> (dto.frontiers_confirmation);
	max_queued_requests = dto.max_queued_requests;
	confirm_req_batches_max = dto.confirm_req_batches_max;
	std::copy (std::begin (dto.rep_crawler_weight_minimum), std::end (dto.rep_crawler_weight_minimum), std::begin (rep_crawler_weight_minimum.bytes));
	for (auto i = 0; i < dto.work_peers_count; i++)
	{
		std::string address (reinterpret_cast<const char *> (dto.work_peers[i].address), dto.work_peers[i].address_len);
		work_peers.push_back (std::make_pair (address, dto.work_peers[i].port));
	}
	for (auto i = 0; i < dto.secondary_work_peers_count; i++)
	{
		std::string address (reinterpret_cast<const char *> (dto.secondary_work_peers[i].address), dto.secondary_work_peers[i].address_len);
		secondary_work_peers.push_back (std::make_pair (address, dto.secondary_work_peers[i].port));
	}
	for (auto i = 0; i < dto.preconfigured_peers_count; i++)
	{
		std::string address (reinterpret_cast<const char *> (dto.preconfigured_peers[i].address), dto.preconfigured_peers[i].address_len);
		preconfigured_peers.push_back (address);
	}
	for (auto i = 0; i < dto.preconfigured_representatives_count; i++)
	{
		nano::account a;
		std::copy (std::begin (dto.preconfigured_representatives[i]), std::end (dto.preconfigured_representatives[i]), std::begin (a.bytes));
		preconfigured_representatives.push_back (a);
	}
	max_pruning_age = std::chrono::seconds (dto.max_pruning_age_s);
	max_pruning_depth = dto.max_pruning_depth;
	callback_address = std::string (reinterpret_cast<const char *> (dto.callback_address), dto.callback_address_len);
	callback_target = std::string (reinterpret_cast<const char *> (dto.callback_target), dto.callback_target_len);
	callback_port = dto.callback_port;
	websocket_config.load_dto (dto.websocket_config);
}

rsnano::NodeConfigDto to_node_config_dto (nano::node_config const & config)
{
	rsnano::NodeConfigDto dto;
	dto.peering_port = config.peering_port;
	dto.bootstrap_fraction_numerator = config.bootstrap_fraction_numerator;
	std::copy (std::begin (config.receive_minimum.bytes), std::end (config.receive_minimum.bytes), std::begin (dto.receive_minimum));
	std::copy (std::begin (config.online_weight_minimum.bytes), std::end (config.online_weight_minimum.bytes), std::begin (dto.online_weight_minimum));
	dto.election_hint_weight_percent = config.election_hint_weight_percent;
	dto.password_fanout = config.password_fanout;
	dto.io_threads = config.io_threads;
	dto.network_threads = config.network_threads;
	dto.work_threads = config.work_threads;
	dto.signature_checker_threads = config.signature_checker_threads;
	dto.enable_voting = config.enable_voting;
	dto.bootstrap_connections = config.bootstrap_connections;
	dto.bootstrap_connections_max = config.bootstrap_connections_max;
	dto.bootstrap_initiator_threads = config.bootstrap_initiator_threads;
	dto.bootstrap_frontier_request_count = config.bootstrap_frontier_request_count;
	dto.block_processor_batch_max_time_ms = config.block_processor_batch_max_time.count ();
	dto.allow_local_peers = config.allow_local_peers;
	std::copy (std::begin (config.vote_minimum.bytes), std::end (config.vote_minimum.bytes), std::begin (dto.vote_minimum));
	dto.vote_generator_delay_ms = config.vote_generator_delay.count ();
	dto.vote_generator_threshold = config.vote_generator_threshold;
	dto.unchecked_cutoff_time_s = config.unchecked_cutoff_time.count ();
	dto.tcp_io_timeout_s = config.tcp_io_timeout.count ();
	dto.pow_sleep_interval_ns = config.pow_sleep_interval.count ();
	std::copy (config.external_address.begin (), config.external_address.end (), std::begin (dto.external_address));
	dto.external_address_len = config.external_address.length ();
	dto.external_port = config.external_port;
	dto.tcp_incoming_connections_max = config.tcp_incoming_connections_max;
	dto.use_memory_pools = config.use_memory_pools;
	dto.confirmation_history_size = config.confirmation_history_size;
	dto.active_elections_size = config.active_elections_size;
	dto.bandwidth_limit = config.bandwidth_limit;
	dto.bandwidth_limit_burst_ratio = config.bandwidth_limit_burst_ratio;
	dto.conf_height_processor_batch_min_time_ms = config.conf_height_processor_batch_min_time.count ();
	dto.backup_before_upgrade = config.backup_before_upgrade;
	dto.max_work_generate_multiplier = config.max_work_generate_multiplier;
	dto.frontiers_confirmation = static_cast<uint8_t> (config.frontiers_confirmation);
	dto.max_queued_requests = config.max_queued_requests;
	dto.confirm_req_batches_max = config.confirm_req_batches_max;
	std::copy (std::begin (config.rep_crawler_weight_minimum.bytes), std::end (config.rep_crawler_weight_minimum.bytes), std::begin (dto.rep_crawler_weight_minimum));
	dto.work_peers_count = config.work_peers.size ();
	for (auto i = 0; i < config.work_peers.size (); i++)
	{
		std::copy (config.work_peers[i].first.begin (), config.work_peers[i].first.end (), std::begin (dto.work_peers[i].address));
		dto.work_peers[i].address_len = config.work_peers[i].first.size ();
		dto.work_peers[i].port = config.work_peers[i].second;
	}
	dto.secondary_work_peers_count = config.secondary_work_peers.size ();
	for (auto i = 0; i < config.secondary_work_peers.size (); i++)
	{
		std::copy (config.secondary_work_peers[i].first.begin (), config.secondary_work_peers[i].first.end (), std::begin (dto.secondary_work_peers[i].address));
		dto.secondary_work_peers[i].address_len = config.secondary_work_peers[i].first.size ();
		dto.secondary_work_peers[i].port = config.secondary_work_peers[i].second;
	}
	dto.preconfigured_peers_count = config.preconfigured_peers.size ();
	for (auto i = 0; i < config.preconfigured_peers.size (); i++)
	{
		std::copy (config.preconfigured_peers[i].begin (), config.preconfigured_peers[i].end (), std::begin (dto.preconfigured_peers[i].address));
		dto.preconfigured_peers[i].address_len = config.preconfigured_peers[i].size ();
	}
	for (auto i = 0; i < config.preconfigured_representatives.size (); i++)
	{
		std::copy (std::begin (config.preconfigured_representatives[i].bytes), std::end (config.preconfigured_representatives[i].bytes), std::begin (dto.preconfigured_representatives[i]));
		dto.preconfigured_representatives_count = config.preconfigured_representatives.size ();
	}
	dto.preconfigured_representatives_count = config.preconfigured_representatives.size ();
	dto.max_pruning_age_s = config.max_pruning_age.count ();
	dto.max_pruning_depth = config.max_pruning_depth;
	std::copy (config.callback_address.begin (), config.callback_address.end (), std::begin (dto.callback_address));
	dto.callback_address_len = config.callback_address.size ();
	std::copy (config.callback_target.begin (), config.callback_target.end (), std::begin (dto.callback_target));
	dto.callback_target_len = config.callback_target.size ();
	return dto;
}

nano::error nano::node_config::serialize_toml (nano::tomlconfig & toml) const
{
	auto dto{ to_node_config_dto (*this) };
	if (rsnano::rsn_node_config_serialize_toml (&dto, &toml) < 0)
		throw std::runtime_error ("could not TOML serialize node_config");

	nano::tomlconfig ipc_l;
	ipc_config.serialize_toml (ipc_l);
	toml.put_child ("ipc", ipc_l);

	nano::tomlconfig diagnostics_l;
	diagnostics_config.serialize_toml (diagnostics_l);
	toml.put_child ("diagnostics", diagnostics_l);

	nano::tomlconfig stat_l;
	stat_config.serialize_toml (stat_l);
	toml.put_child ("statistics", stat_l);

	nano::tomlconfig rocksdb_l;
	rocksdb_config.serialize_toml (rocksdb_l);
	toml.put_child ("rocksdb", rocksdb_l);

	nano::tomlconfig lmdb_l;
	lmdb_config.serialize_toml (lmdb_l);
	toml.put_child ("lmdb", lmdb_l);

	return toml.get_error ();
}

nano::error nano::node_config::deserialize_toml (nano::tomlconfig & toml)
{
	try
	{
		if (toml.has_key ("httpcallback"))
		{
			auto callback_l (toml.get_required_child ("httpcallback"));
			callback_l.get<std::string> ("address", callback_address);
			callback_l.get<uint16_t> ("port", callback_port);
			callback_l.get<std::string> ("target", callback_target);
		}

		if (toml.has_key ("logging"))
		{
			auto logging_l (toml.get_required_child ("logging"));
			logging.deserialize_toml (logging_l);
		}

		if (toml.has_key ("websocket"))
		{
			auto websocket_config_l (toml.get_required_child ("websocket"));
			websocket_config.deserialize_toml (websocket_config_l);
		}

		if (toml.has_key ("ipc"))
		{
			auto ipc_config_l (toml.get_required_child ("ipc"));
			ipc_config.deserialize_toml (ipc_config_l);
		}

		if (toml.has_key ("diagnostics"))
		{
			auto diagnostics_config_l (toml.get_required_child ("diagnostics"));
			diagnostics_config.deserialize_toml (diagnostics_config_l);
		}

		if (toml.has_key ("statistics"))
		{
			auto stat_config_l (toml.get_required_child ("statistics"));
			stat_config.deserialize_toml (stat_config_l);
		}

		if (toml.has_key ("rocksdb"))
		{
			auto rocksdb_config_l (toml.get_required_child ("rocksdb"));
			rocksdb_config.deserialize_toml (rocksdb_config_l);
		}

		if (toml.has_key ("work_peers"))
		{
			work_peers.clear ();
			toml.array_entries_required<std::string> ("work_peers", [this] (std::string const & entry_a) {
				this->deserialize_address (entry_a, this->work_peers);
			});
		}

		if (toml.has_key (preconfigured_peers_key))
		{
			preconfigured_peers.clear ();
			toml.array_entries_required<std::string> (preconfigured_peers_key, [this] (std::string entry) {
				preconfigured_peers.push_back (entry);
			});
		}

		if (toml.has_key ("preconfigured_representatives"))
		{
			preconfigured_representatives.clear ();
			toml.array_entries_required<std::string> ("preconfigured_representatives", [this, &toml] (std::string entry) {
				nano::account representative{};
				if (representative.decode_account (entry))
				{
					toml.get_error ().set ("Invalid representative account: " + entry);
				}
				preconfigured_representatives.push_back (representative);
			});
		}

		if (preconfigured_representatives.empty ())
		{
			toml.get_error ().set ("At least one representative account must be set");
		}

		auto receive_minimum_l (receive_minimum.to_string_dec ());
		if (toml.has_key ("receive_minimum"))
		{
			receive_minimum_l = toml.get<std::string> ("receive_minimum");
		}
		if (receive_minimum.decode_dec (receive_minimum_l))
		{
			toml.get_error ().set ("receive_minimum contains an invalid decimal amount");
		}

		auto online_weight_minimum_l (online_weight_minimum.to_string_dec ());
		if (toml.has_key ("online_weight_minimum"))
		{
			online_weight_minimum_l = toml.get<std::string> ("online_weight_minimum");
		}
		if (online_weight_minimum.decode_dec (online_weight_minimum_l))
		{
			toml.get_error ().set ("online_weight_minimum contains an invalid decimal amount");
		}

		auto vote_minimum_l (vote_minimum.to_string_dec ());
		if (toml.has_key ("vote_minimum"))
		{
			vote_minimum_l = toml.get<std::string> ("vote_minimum");
		}
		if (vote_minimum.decode_dec (vote_minimum_l))
		{
			toml.get_error ().set ("vote_minimum contains an invalid decimal amount");
		}

		auto delay_l = vote_generator_delay.count ();
		toml.get ("vote_generator_delay", delay_l);
		vote_generator_delay = std::chrono::milliseconds (delay_l);

		toml.get<unsigned> ("vote_generator_threshold", vote_generator_threshold);

		auto block_processor_batch_max_time_l = block_processor_batch_max_time.count ();
		toml.get ("block_processor_batch_max_time", block_processor_batch_max_time_l);
		block_processor_batch_max_time = std::chrono::milliseconds (block_processor_batch_max_time_l);

		auto unchecked_cutoff_time_l = static_cast<unsigned long> (unchecked_cutoff_time.count ());
		toml.get ("unchecked_cutoff_time", unchecked_cutoff_time_l);
		unchecked_cutoff_time = std::chrono::seconds (unchecked_cutoff_time_l);

		auto tcp_io_timeout_l = static_cast<unsigned long> (tcp_io_timeout.count ());
		toml.get ("tcp_io_timeout", tcp_io_timeout_l);
		tcp_io_timeout = std::chrono::seconds (tcp_io_timeout_l);

		toml.get<uint16_t> ("peering_port", peering_port);
		toml.get<unsigned> ("bootstrap_fraction_numerator", bootstrap_fraction_numerator);
		toml.get<unsigned> ("election_hint_weight_percent", election_hint_weight_percent);
		toml.get<unsigned> ("password_fanout", password_fanout);
		toml.get<unsigned> ("io_threads", io_threads);
		toml.get<unsigned> ("work_threads", work_threads);
		toml.get<unsigned> ("network_threads", network_threads);
		toml.get<unsigned> ("bootstrap_connections", bootstrap_connections);
		toml.get<unsigned> ("bootstrap_connections_max", bootstrap_connections_max);
		toml.get<unsigned> ("bootstrap_initiator_threads", bootstrap_initiator_threads);
		toml.get<uint32_t> ("bootstrap_frontier_request_count", bootstrap_frontier_request_count);
		toml.get<bool> ("enable_voting", enable_voting);
		toml.get<bool> ("allow_local_peers", allow_local_peers);
		toml.get<unsigned> (signature_checker_threads_key, signature_checker_threads);

		if (toml.has_key ("lmdb"))
		{
			auto lmdb_config_l (toml.get_required_child ("lmdb"));
			lmdb_config.deserialize_toml (lmdb_config_l);
		}

		boost::asio::ip::address_v6 external_address_l;
		toml.get<boost::asio::ip::address_v6> ("external_address", external_address_l);
		external_address = external_address_l.to_string ();
		toml.get<uint16_t> ("external_port", external_port);
		toml.get<unsigned> ("tcp_incoming_connections_max", tcp_incoming_connections_max);

		auto pow_sleep_interval_l (pow_sleep_interval.count ());
		toml.get (pow_sleep_interval_key, pow_sleep_interval_l);
		pow_sleep_interval = std::chrono::nanoseconds (pow_sleep_interval_l);
		toml.get<bool> ("use_memory_pools", use_memory_pools);
		toml.get<std::size_t> ("confirmation_history_size", confirmation_history_size);
		toml.get<std::size_t> ("active_elections_size", active_elections_size);
		toml.get<std::size_t> ("bandwidth_limit", bandwidth_limit);
		toml.get<double> ("bandwidth_limit_burst_ratio", bandwidth_limit_burst_ratio);
		toml.get<bool> ("backup_before_upgrade", backup_before_upgrade);

		auto conf_height_processor_batch_min_time_l (conf_height_processor_batch_min_time.count ());
		toml.get ("conf_height_processor_batch_min_time", conf_height_processor_batch_min_time_l);
		conf_height_processor_batch_min_time = std::chrono::milliseconds (conf_height_processor_batch_min_time_l);

		toml.get<double> ("max_work_generate_multiplier", max_work_generate_multiplier);

		toml.get<uint32_t> ("max_queued_requests", max_queued_requests);
		toml.get<uint32_t> ("confirm_req_batches_max", confirm_req_batches_max);

		auto rep_crawler_weight_minimum_l (rep_crawler_weight_minimum.to_string_dec ());
		if (toml.has_key ("rep_crawler_weight_minimum"))
		{
			rep_crawler_weight_minimum_l = toml.get<std::string> ("rep_crawler_weight_minimum");
		}
		if (rep_crawler_weight_minimum.decode_dec (rep_crawler_weight_minimum_l))
		{
			toml.get_error ().set ("rep_crawler_weight_minimum contains an invalid decimal amount");
		}

		if (toml.has_key ("frontiers_confirmation"))
		{
			auto frontiers_confirmation_l (toml.get<std::string> ("frontiers_confirmation"));
			frontiers_confirmation = deserialize_frontiers_confirmation (frontiers_confirmation_l);
		}

		if (toml.has_key ("experimental"))
		{
			auto experimental_config_l (toml.get_required_child ("experimental"));
			if (experimental_config_l.has_key ("secondary_work_peers"))
			{
				secondary_work_peers.clear ();
				experimental_config_l.array_entries_required<std::string> ("secondary_work_peers", [this] (std::string const & entry_a) {
					this->deserialize_address (entry_a, this->secondary_work_peers);
				});
			}
			auto max_pruning_age_l (max_pruning_age.count ());
			experimental_config_l.get ("max_pruning_age", max_pruning_age_l);
			max_pruning_age = std::chrono::seconds (max_pruning_age_l);
			experimental_config_l.get<uint64_t> ("max_pruning_depth", max_pruning_depth);
		}

		// Validate ranges
		if (election_hint_weight_percent < 5 || election_hint_weight_percent > 50)
		{
			toml.get_error ().set ("election_hint_weight_percent must be a number between 5 and 50");
		}
		if (password_fanout < 16 || password_fanout > 1024 * 1024)
		{
			toml.get_error ().set ("password_fanout must be a number between 16 and 1048576");
		}
		if (io_threads == 0)
		{
			toml.get_error ().set ("io_threads must be non-zero");
		}
		if (active_elections_size <= 250 && !network_params.network.is_dev_network ())
		{
			toml.get_error ().set ("active_elections_size must be greater than 250");
		}
		if (bandwidth_limit > std::numeric_limits<std::size_t>::max ())
		{
			toml.get_error ().set ("bandwidth_limit unbounded = 0, default = 10485760, max = 18446744073709551615");
		}
		if (vote_generator_threshold < 1 || vote_generator_threshold > 11)
		{
			toml.get_error ().set ("vote_generator_threshold must be a number between 1 and 11");
		}
		if (max_work_generate_multiplier < 1)
		{
			toml.get_error ().set ("max_work_generate_multiplier must be greater than or equal to 1");
		}
		if (frontiers_confirmation == nano::frontiers_confirmation_mode::invalid)
		{
			toml.get_error ().set ("frontiers_confirmation value is invalid (available: always, auto, disabled)");
		}
		if (block_processor_batch_max_time < network_params.node.process_confirmed_interval)
		{
			toml.get_error ().set ((boost::format ("block_processor_batch_max_time value must be equal or larger than %1%ms") % network_params.node.process_confirmed_interval.count ()).str ());
		}
		if (max_pruning_age < std::chrono::seconds (5 * 60) && !network_params.network.is_dev_network ())
		{
			toml.get_error ().set ("max_pruning_age must be greater than or equal to 5 minutes");
		}
		if (confirm_req_batches_max < 1 || confirm_req_batches_max > 100)
		{
			toml.get_error ().set ("confirm_req_batches_max must be between 1 and 100");
		}
		if (bootstrap_frontier_request_count < 1024)
		{
			toml.get_error ().set ("bootstrap_frontier_request_count must be greater than or equal to 1024");
		}
	}
	catch (std::runtime_error const & ex)
	{
		toml.get_error ().set (ex.what ());
	}

	return toml.get_error ();
}

nano::error nano::node_config::serialize_json (nano::jsonconfig & json) const
{
	json.put ("version", json_version ());
	json.put ("peering_port", peering_port);
	json.put ("bootstrap_fraction_numerator", bootstrap_fraction_numerator);
	json.put ("receive_minimum", receive_minimum.to_string_dec ());

	nano::jsonconfig logging_l;
	logging.serialize_json (logging_l);
	json.put_child ("logging", logging_l);

	nano::jsonconfig work_peers_l;
	for (auto i (work_peers.begin ()), n (work_peers.end ()); i != n; ++i)
	{
		work_peers_l.push (boost::str (boost::format ("%1%:%2%") % i->first % i->second));
	}
	json.put_child ("work_peers", work_peers_l);
	nano::jsonconfig preconfigured_peers_l;
	for (auto i (preconfigured_peers.begin ()), n (preconfigured_peers.end ()); i != n; ++i)
	{
		preconfigured_peers_l.push (*i);
	}
	json.put_child (preconfigured_peers_key, preconfigured_peers_l);

	nano::jsonconfig preconfigured_representatives_l;
	for (auto i (preconfigured_representatives.begin ()), n (preconfigured_representatives.end ()); i != n; ++i)
	{
		preconfigured_representatives_l.push (i->to_account ());
	}
	json.put_child ("preconfigured_representatives", preconfigured_representatives_l);

	json.put ("online_weight_minimum", online_weight_minimum.to_string_dec ());
	json.put ("password_fanout", password_fanout);
	json.put ("io_threads", io_threads);
	json.put ("network_threads", network_threads);
	json.put ("work_threads", work_threads);
	json.put (signature_checker_threads_key, signature_checker_threads);
	json.put ("enable_voting", enable_voting);
	json.put ("bootstrap_connections", bootstrap_connections);
	json.put ("bootstrap_connections_max", bootstrap_connections_max);
	json.put ("callback_address", callback_address);
	json.put ("callback_port", callback_port);
	json.put ("callback_target", callback_target);
	json.put ("block_processor_batch_max_time", block_processor_batch_max_time.count ());
	json.put ("allow_local_peers", allow_local_peers);
	json.put ("vote_minimum", vote_minimum.to_string_dec ());
	json.put ("vote_generator_delay", vote_generator_delay.count ());
	json.put ("vote_generator_threshold", vote_generator_threshold);
	json.put ("unchecked_cutoff_time", unchecked_cutoff_time.count ());
	json.put ("tcp_io_timeout", tcp_io_timeout.count ());
	json.put ("pow_sleep_interval", pow_sleep_interval.count ());
	json.put ("external_address", external_address);
	json.put ("external_port", external_port);
	json.put ("tcp_incoming_connections_max", tcp_incoming_connections_max);
	json.put ("use_memory_pools", use_memory_pools);
	json.put ("rep_crawler_weight_minimum", online_weight_minimum.to_string_dec ());
	nano::jsonconfig websocket_l;
	websocket_config.serialize_json (websocket_l);
	json.put_child ("websocket", websocket_l);
	nano::jsonconfig ipc_l;
	ipc_config.serialize_json (ipc_l);
	json.put_child ("ipc", ipc_l);
	nano::jsonconfig diagnostics_l;
	diagnostics_config.serialize_json (diagnostics_l);
	json.put_child ("diagnostics", diagnostics_l);
	json.put ("confirmation_history_size", confirmation_history_size);
	json.put ("active_elections_size", active_elections_size);
	json.put ("bandwidth_limit", bandwidth_limit);
	json.put ("backup_before_upgrade", backup_before_upgrade);
	json.put ("work_watcher_period", 5);

	return json.get_error ();
}

bool nano::node_config::upgrade_json (unsigned version_a, nano::jsonconfig & json)
{
	json.put ("version", json_version ());
	switch (version_a)
	{
		case 1:
		case 2:
		case 3:
		case 4:
		case 5:
		case 6:
		case 7:
		case 8:
		case 9:
		case 10:
		case 11:
		case 12:
		case 13:
		case 14:
		case 15:
		case 16:
			throw std::runtime_error ("node_config version unsupported for upgrade. Upgrade to a v19, v20 or v21 node first, or delete the config and ledger files");
		case 17:
		{
			json.put ("active_elections_size", 10000); // Update value
			json.put ("vote_generator_delay", 100); // Update value
			json.put ("backup_before_upgrade", backup_before_upgrade);
			json.put ("work_watcher_period", 5);
		}
		case 18:
			break;
		default:
			throw std::runtime_error ("Unknown node_config version");
	}
	return version_a < json_version ();
}

nano::error nano::node_config::deserialize_json (bool & upgraded_a, nano::jsonconfig & json)
{
	try
	{
		auto version_l (json.get<unsigned> ("version"));
		upgraded_a |= upgrade_json (version_l, json);

		auto logging_l (json.get_required_child ("logging"));
		logging.deserialize_json (upgraded_a, logging_l);

		work_peers.clear ();
		auto work_peers_l (json.get_required_child ("work_peers"));
		work_peers_l.array_entries<std::string> ([this] (std::string entry) {
			auto port_position (entry.rfind (':'));
			bool result = port_position == -1;
			if (!result)
			{
				auto port_str (entry.substr (port_position + 1));
				uint16_t port;
				result |= parse_port (port_str, port);
				if (!result)
				{
					auto address (entry.substr (0, port_position));
					this->work_peers.emplace_back (address, port);
				}
			}
		});

		auto preconfigured_peers_l (json.get_required_child (preconfigured_peers_key));
		preconfigured_peers.clear ();
		preconfigured_peers_l.array_entries<std::string> ([this] (std::string entry) {
			preconfigured_peers.push_back (entry);
		});

		auto preconfigured_representatives_l (json.get_required_child ("preconfigured_representatives"));
		preconfigured_representatives.clear ();
		preconfigured_representatives_l.array_entries<std::string> ([this, &json] (std::string entry) {
			nano::account representative{};
			if (representative.decode_account (entry))
			{
				json.get_error ().set ("Invalid representative account: " + entry);
			}
			preconfigured_representatives.push_back (representative);
		});

		if (preconfigured_representatives.empty ())
		{
			json.get_error ().set ("At least one representative account must be set");
		}
		auto stat_config_l (json.get_optional_child ("statistics"));
		if (stat_config_l)
		{
			stat_config.deserialize_json (stat_config_l.get ());
		}

		auto receive_minimum_l (json.get<std::string> ("receive_minimum"));
		if (receive_minimum.decode_dec (receive_minimum_l))
		{
			json.get_error ().set ("receive_minimum contains an invalid decimal amount");
		}

		auto online_weight_minimum_l (json.get<std::string> ("online_weight_minimum"));
		if (online_weight_minimum.decode_dec (online_weight_minimum_l))
		{
			json.get_error ().set ("online_weight_minimum contains an invalid decimal amount");
		}

		auto rep_crawler_weight_minimum_l (json.get<std::string> ("rep_crawler_weight_minimum"));
		if (rep_crawler_weight_minimum.decode_dec (rep_crawler_weight_minimum_l))
		{
			json.get_error ().set ("rep_crawler_weight_minimum contains an invalid decimal amount");
		}

		auto vote_minimum_l (json.get<std::string> ("vote_minimum"));
		if (vote_minimum.decode_dec (vote_minimum_l))
		{
			json.get_error ().set ("vote_minimum contains an invalid decimal amount");
		}

		auto delay_l = vote_generator_delay.count ();
		json.get ("vote_generator_delay", delay_l);
		vote_generator_delay = std::chrono::milliseconds (delay_l);

		json.get<unsigned> ("vote_generator_threshold", vote_generator_threshold);

		auto block_processor_batch_max_time_l (json.get<unsigned long> ("block_processor_batch_max_time"));
		block_processor_batch_max_time = std::chrono::milliseconds (block_processor_batch_max_time_l);
		auto unchecked_cutoff_time_l = static_cast<unsigned long> (unchecked_cutoff_time.count ());
		json.get ("unchecked_cutoff_time", unchecked_cutoff_time_l);
		unchecked_cutoff_time = std::chrono::seconds (unchecked_cutoff_time_l);

		auto tcp_io_timeout_l = static_cast<unsigned long> (tcp_io_timeout.count ());
		json.get ("tcp_io_timeout", tcp_io_timeout_l);
		tcp_io_timeout = std::chrono::seconds (tcp_io_timeout_l);

		auto ipc_config_l (json.get_optional_child ("ipc"));
		if (ipc_config_l)
		{
			ipc_config.deserialize_json (upgraded_a, ipc_config_l.get ());
		}
		auto websocket_config_l (json.get_optional_child ("websocket"));
		if (websocket_config_l)
		{
			websocket_config.deserialize_json (websocket_config_l.get ());
		}
		auto diagnostics_config_l (json.get_optional_child ("diagnostics"));
		if (diagnostics_config_l)
		{
			diagnostics_config.deserialize_json (diagnostics_config_l.get ());
		}
		json.get<uint16_t> ("peering_port", peering_port);
		json.get<unsigned> ("bootstrap_fraction_numerator", bootstrap_fraction_numerator);
		json.get<unsigned> ("password_fanout", password_fanout);
		json.get<unsigned> ("io_threads", io_threads);
		json.get<unsigned> ("work_threads", work_threads);
		json.get<unsigned> ("network_threads", network_threads);
		json.get<unsigned> ("bootstrap_connections", bootstrap_connections);
		json.get<unsigned> ("bootstrap_connections_max", bootstrap_connections_max);
		json.get<std::string> ("callback_address", callback_address);
		json.get<uint16_t> ("callback_port", callback_port);
		json.get<std::string> ("callback_target", callback_target);
		json.get<bool> ("enable_voting", enable_voting);
		json.get<bool> ("allow_local_peers", allow_local_peers);
		json.get<unsigned> (signature_checker_threads_key, signature_checker_threads);
		boost::asio::ip::address_v6 external_address_l;
		json.get<boost::asio::ip::address_v6> ("external_address", external_address_l);
		external_address = external_address_l.to_string ();
		json.get<uint16_t> ("external_port", external_port);
		json.get<unsigned> ("tcp_incoming_connections_max", tcp_incoming_connections_max);

		auto pow_sleep_interval_l (pow_sleep_interval.count ());
		json.get (pow_sleep_interval_key, pow_sleep_interval_l);
		pow_sleep_interval = std::chrono::nanoseconds (pow_sleep_interval_l);
		json.get<bool> ("use_memory_pools", use_memory_pools);
		json.get<std::size_t> ("confirmation_history_size", confirmation_history_size);
		json.get<std::size_t> ("active_elections_size", active_elections_size);
		json.get<std::size_t> ("bandwidth_limit", bandwidth_limit);
		json.get<bool> ("backup_before_upgrade", backup_before_upgrade);

		auto conf_height_processor_batch_min_time_l (conf_height_processor_batch_min_time.count ());
		json.get ("conf_height_processor_batch_min_time", conf_height_processor_batch_min_time_l);
		conf_height_processor_batch_min_time = std::chrono::milliseconds (conf_height_processor_batch_min_time_l);

		// Validate ranges
		if (password_fanout < 16 || password_fanout > 1024 * 1024)
		{
			json.get_error ().set ("password_fanout must be a number between 16 and 1048576");
		}
		if (io_threads == 0)
		{
			json.get_error ().set ("io_threads must be non-zero");
		}
		if (active_elections_size <= 250 && !network_params.network.is_dev_network ())
		{
			json.get_error ().set ("active_elections_size must be greater than 250");
		}
		if (bandwidth_limit > std::numeric_limits<std::size_t>::max ())
		{
			json.get_error ().set ("bandwidth_limit unbounded = 0, default = 10485760, max = 18446744073709551615");
		}
		if (vote_generator_threshold < 1 || vote_generator_threshold > 11)
		{
			json.get_error ().set ("vote_generator_threshold must be a number between 1 and 11");
		}
	}
	catch (std::runtime_error const & ex)
	{
		json.get_error ().set (ex.what ());
	}
	return json.get_error ();
}

nano::frontiers_confirmation_mode nano::node_config::deserialize_frontiers_confirmation (std::string const & string_a)
{
	if (string_a == "always")
	{
		return nano::frontiers_confirmation_mode::always;
	}
	else if (string_a == "auto")
	{
		return nano::frontiers_confirmation_mode::automatic;
	}
	else if (string_a == "disabled")
	{
		return nano::frontiers_confirmation_mode::disabled;
	}
	else
	{
		return nano::frontiers_confirmation_mode::invalid;
	}
}

void nano::node_config::deserialize_address (std::string const & entry_a, std::vector<std::pair<std::string, uint16_t>> & container_a) const
{
	auto port_position (entry_a.rfind (':'));
	bool result = (port_position == -1);
	if (!result)
	{
		auto port_str (entry_a.substr (port_position + 1));
		uint16_t port;
		result |= parse_port (port_str, port);
		if (!result)
		{
			auto address (entry_a.substr (0, port_position));
			container_a.emplace_back (address, port);
		}
	}
}

nano::account nano::node_config::random_representative () const
{
	debug_assert (!preconfigured_representatives.empty ());
	std::size_t index (nano::random_pool::generate_word32 (0, static_cast<CryptoPP::word32> (preconfigured_representatives.size () - 1)));
	auto result (preconfigured_representatives[index]);
	return result;
}
