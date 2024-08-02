use super::{NodeRpcToml, NodeToml, OpenclToml};
use crate::config::DaemonConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct DaemonToml {
    pub node: Option<NodeToml>,
    pub rpc: Option<NodeRpcToml>,
    pub opencl: Option<OpenclToml>,
}

impl From<&DaemonToml> for DaemonConfig {
    fn from(toml: &DaemonToml) -> Self {
        let mut config = DaemonConfig::default();
        if let Some(node_toml) = &toml.node {
            config.node = node_toml.into();
        }
        config
    }
}

impl From<&DaemonConfig> for DaemonToml {
    fn from(config: &DaemonConfig) -> Self {
        Self {
            node: Some((&config.node).into()),
            rpc: Some(config.into()),
            opencl: Some(config.into()),
        }
    }
}

impl From<&DaemonConfig> for NodeRpcToml {
    fn from(config: &DaemonConfig) -> Self {
        Self {
            enable: Some(config.rpc_enable),
            enable_sign_hash: Some(config.rpc.enable_sign_hash),
            child_process: Some((&config.rpc.child_process).into()),
        }
    }
}

impl From<&DaemonConfig> for OpenclToml {
    fn from(config: &DaemonConfig) -> Self {
        Self {
            enable: Some(config.opencl_enable),
            platform: Some(config.opencl.platform),
            device: Some(config.opencl.device),
            threads: Some(config.opencl.threads),
        }
    }
}

impl Default for DaemonToml {
    fn default() -> Self {
        Self {
            node: Some(NodeToml::default()),
            opencl: Some(OpenclToml::default()),
            rpc: Some(NodeRpcToml::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{DaemonConfig, DaemonToml},
        nullable_fs::NullableFilesystem,
    };
    use std::path::PathBuf;

    #[test]
    fn toml_serialize_defaults() {
        let default_toml_str = r#"
            [node]
            allow_local_peers = true
            background_threads = 8
            backlog_scan_batch_size = 10000
            backlog_scan_frequency = 10
            backup_before_upgrade = false
            bandwidth_limit = 10485760
            bandwidth_limit_burst_ratio = 3.0
            block_processor_batch_max_time = 500
            bootstrap_bandwidth_burst_ratio = 1.0
            bootstrap_bandwidth_limit = 5242880
            bootstrap_connections = 4
            bootstrap_connections_max = 64
            bootstrap_fraction_numerator = 1
            bootstrap_frontier_request_count = 1048576
            bootstrap_initiator_threads = 1
            bootstrap_serving_threads = 1
            confirming_set_batch_time = 250
            enable_voting = false
            external_address = "::"
            external_port = 0
            frontiers_confirmation = "auto"
            io_threads = 8
            max_queued_requests = 512
            max_work_generate_multiplier = 64.0
            network_threads = 8
            online_weight_minimum = "60000000000000000000000000000000000000"
            password_fanout = 1024
            peering_port = 54000
            pow_sleep_interval = 0
            preconfigured_peers = ["peering-beta.nano.org"]
            preconfigured_representatives = ["nano_1defau1t9off1ine9rep99999999999999999999999999999999wgmuzxxy"]
            receive_minimum = "1000000000000000000000000"
            rep_crawler_weight_minimum = "340282366920938463463374607431768211455"
            representative_vote_weight_minimum = "10000000000000000000000000000000"
            request_aggregator_threads = 8
            signature_checker_threads = 4
            tcp_incoming_connections_max = 2048
            tcp_io_timeout = 15
            unchecked_cutoff_time = 14400
            use_memory_pools = true
            vote_generator_delay = 100
            vote_generator_threshold = 3
            vote_minimum = "1000000000000000000000000000000000"
            work_threads = 8

            [node.active_elections]
            confirmation_cache = 65536
            confirmation_history_size = 2048
            hinted_limit_percentage = 20
            optimistic_limit_percentage = 10
            size = 5000

            [node.block_processor]
            max_peer_queue = 128
            max_system_queue = 16384
            priority_bootstrap = 8
            priority_live = 1
            priority_local = 16

            [node.bootstrap_ascending]
            block_wait_count = 1000
            database_requests_limit = 1024
            pull_count = 128
            requests_limit = 64
            throttle_coefficient = 16
            throttle_wait = 100
            timeout = 3000

            [node.bootstrap_ascending.account_sets]
            blocking_max = 262144
            consideration_count = 4
            cooldown = 3000
            priorities_max = 262144

            [node.bootstrap_server]
            batch_size = 64
            max_queue = 16
            threads = 1

            [node.diagnostics.txn_tracking]
            enable = false
            ignore_writes_below_block_processor_max_time = true
            min_read_txn_time = 5000
            min_write_txn_time = 500

            [node.experimental]
            max_pruning_age = 300
            max_pruning_depth = 0
            secondary_work_peers = ["127.0.0.1:8076"]

            [node.httpcallback]
            address = ""
            port = 0
            target = ""

            [node.ipc.flatbuffers]
            skip_unexpected_fields_in_json = true
            verify_buffers = true

            [node.ipc.local]
            allow_unsafe = false
            enable = false
            io_timeout = 15
            path = "/tmp/nano"

            [node.ipc.tcp]
            enable = false
            io_timeout = 15
            port = 56000

            [node.lmdb]
            map_size = 274877906944
            max_databases = 128
            sync = "always"

            [node.message_processor]
            max_queue = 64
            threads = 2

            [node.monitor]
            enable = true
            interval = 60

            [node.optimistic_scheduler]
            enable = true
            gap_threshold = 32
            max_size = 65536

            [node.priority_bucket]
            max_blocks = 8192
            max_elections = 150
            reserved_elections = 100

            [node.rep_crawler]
            query_timeout = 60000

            [node.request_aggregator]
            batch_size = 16
            max_queue = 128
            threads = 4

            [node.statistics]
            max_samples = 16384

            [node.statistics.log]
            filename_counters = "counters.stat"
            filename_samples = "samples.stat"
            headers = true
            interval_counters = 0
            interval_samples = 0
            rotation_count = 100

            [node.vote_cache]
            age_cutoff = 900
            max_size = 65536
            max_voters = 64

            [node.vote_processor]
            batch_size = 1024
            max_non_pr_queue = 32
            max_pr_queue = 256
            pr_priority = 3
            threads = 4

            [node.websocket]
            address = "::1"
            enable = false
            port = 57000

            [opencl]
            device = 0
            enable = false
            platform = 0
            threads = 1048576

            [rpc]
            enable = false
            enable_sign_hash = false

            [rpc.child_process]
            enable = false
            rpc_path = "/Users/ruimorais/rsnano/rust/../build/cargo/debug/deps/nano_rpc"
        "#;

        let default_daemon_toml = DaemonToml::default();
        let deserialized_toml: DaemonToml = toml::from_str(&default_toml_str).unwrap();

        let default_node = default_daemon_toml.node.unwrap();
        let deserialized_node = deserialized_toml.node.unwrap();

        assert_eq!(
            default_node.allow_local_peers,
            deserialized_node.allow_local_peers
        );
        assert_eq!(
            default_node.background_threads,
            deserialized_node.background_threads
        );
        assert_eq!(
            default_node.backlog_scan_batch_size,
            deserialized_node.backlog_scan_batch_size
        );
        assert_eq!(
            default_node.backlog_scan_frequency,
            deserialized_node.backlog_scan_frequency
        );
        assert_eq!(
            default_node.backup_before_upgrade,
            deserialized_node.backup_before_upgrade
        );
        assert_eq!(
            default_node.bandwidth_limit,
            deserialized_node.bandwidth_limit
        );
        assert_eq!(
            default_node.bandwidth_limit_burst_ratio,
            deserialized_node.bandwidth_limit_burst_ratio
        );
        assert_eq!(
            default_node.block_processor_batch_max_time,
            deserialized_node.block_processor_batch_max_time
        );
        assert_eq!(
            default_node.bootstrap_bandwidth_burst_ratio,
            deserialized_node.bootstrap_bandwidth_burst_ratio
        );
        assert_eq!(
            default_node.bootstrap_bandwidth_limit,
            deserialized_node.bootstrap_bandwidth_limit
        );
        assert_eq!(
            default_node.bootstrap_connections,
            deserialized_node.bootstrap_connections
        );
        assert_eq!(
            default_node.bootstrap_connections_max,
            deserialized_node.bootstrap_connections_max
        );
        assert_eq!(
            default_node.bootstrap_fraction_numerator,
            deserialized_node.bootstrap_fraction_numerator
        );
        assert_eq!(
            default_node.bootstrap_frontier_request_count,
            deserialized_node.bootstrap_frontier_request_count
        );
        assert_eq!(
            default_node.bootstrap_initiator_threads,
            deserialized_node.bootstrap_initiator_threads
        );
        assert_eq!(
            default_node.bootstrap_serving_threads,
            deserialized_node.bootstrap_serving_threads
        );
        assert_eq!(
            default_node.confirming_set_batch_time,
            deserialized_node.confirming_set_batch_time
        );
        assert_eq!(default_node.enable_voting, deserialized_node.enable_voting);
        assert_eq!(
            default_node.external_address,
            deserialized_node.external_address
        );
        assert_eq!(default_node.external_port, deserialized_node.external_port);
        assert_eq!(
            default_node.frontiers_confirmation,
            deserialized_node.frontiers_confirmation
        );
        assert_eq!(default_node.io_threads, deserialized_node.io_threads);
        assert_eq!(
            default_node.max_queued_requests,
            deserialized_node.max_queued_requests
        );
        assert_eq!(
            default_node.max_work_generate_multiplier,
            deserialized_node.max_work_generate_multiplier
        );
        assert_eq!(
            default_node.network_threads,
            deserialized_node.network_threads
        );
        assert_eq!(
            default_node.online_weight_minimum,
            deserialized_node.online_weight_minimum
        );
        assert_eq!(
            default_node.password_fanout,
            deserialized_node.password_fanout
        );
        assert_eq!(default_node.peering_port, deserialized_node.peering_port);
        assert_eq!(
            default_node.pow_sleep_interval,
            deserialized_node.pow_sleep_interval
        );
        assert_eq!(
            default_node.preconfigured_peers,
            deserialized_node.preconfigured_peers
        );
        assert_eq!(
            default_node.preconfigured_representatives,
            deserialized_node.preconfigured_representatives
        );
        assert_eq!(
            default_node.receive_minimum,
            deserialized_node.receive_minimum
        );
        assert_eq!(
            default_node.rep_crawler_weight_minimum,
            deserialized_node.rep_crawler_weight_minimum
        );
        assert_eq!(
            default_node.representative_vote_weight_minimum,
            deserialized_node.representative_vote_weight_minimum
        );
        assert_eq!(
            default_node.request_aggregator_threads,
            deserialized_node.request_aggregator_threads
        );
        assert_eq!(
            default_node.signature_checker_threads,
            deserialized_node.signature_checker_threads
        );
        assert_eq!(
            default_node.tcp_incoming_connections_max,
            deserialized_node.tcp_incoming_connections_max
        );
        assert_eq!(
            default_node.tcp_io_timeout,
            deserialized_node.tcp_io_timeout
        );
        assert_eq!(
            default_node.unchecked_cutoff_time,
            deserialized_node.unchecked_cutoff_time
        );
        assert_eq!(
            default_node.use_memory_pools,
            deserialized_node.use_memory_pools
        );
        assert_eq!(
            default_node.vote_generator_delay,
            deserialized_node.vote_generator_delay
        );
        assert_eq!(
            default_node.vote_generator_threshold,
            deserialized_node.vote_generator_threshold
        );
        assert_eq!(default_node.vote_minimum, deserialized_node.vote_minimum);
        assert_eq!(default_node.work_threads, deserialized_node.work_threads);

        let default_active_elections = default_node.active_elections.unwrap();
        let deserialized_active_elections = deserialized_node.active_elections.unwrap();

        assert_eq!(
            default_active_elections.confirmation_cache,
            deserialized_active_elections.confirmation_cache
        );
        assert_eq!(
            default_active_elections.confirmation_history_size,
            deserialized_active_elections.confirmation_history_size
        );
        assert_eq!(
            default_active_elections.hinted_limit_percentage,
            deserialized_active_elections.hinted_limit_percentage
        );
        assert_eq!(
            default_active_elections.optimistic_limit_percentage,
            deserialized_active_elections.optimistic_limit_percentage
        );
        assert_eq!(
            default_active_elections.size,
            deserialized_active_elections.size
        );

        let default_block_processor = default_node.block_processor.unwrap();
        let deserialized_block_processor = deserialized_node.block_processor.unwrap();

        assert_eq!(
            default_block_processor.max_peer_queue,
            deserialized_block_processor.max_peer_queue
        );
        assert_eq!(
            default_block_processor.max_system_queue,
            deserialized_block_processor.max_system_queue
        );
        assert_eq!(
            default_block_processor.priority_bootstrap,
            deserialized_block_processor.priority_bootstrap
        );
        assert_eq!(
            default_block_processor.priority_live,
            deserialized_block_processor.priority_live
        );
        assert_eq!(
            default_block_processor.priority_local,
            deserialized_block_processor.priority_local
        );

        let default_bootstrap_ascending = default_node.bootstrap_ascending.unwrap();
        let deserialized_bootstrap_ascending = deserialized_node.bootstrap_ascending.unwrap();

        assert_eq!(
            default_bootstrap_ascending.block_wait_count,
            deserialized_bootstrap_ascending.block_wait_count
        );
        assert_eq!(
            default_bootstrap_ascending.database_requests_limit,
            deserialized_bootstrap_ascending.database_requests_limit
        );
        assert_eq!(
            default_bootstrap_ascending.pull_count,
            deserialized_bootstrap_ascending.pull_count
        );
        assert_eq!(
            default_bootstrap_ascending.requests_limit,
            deserialized_bootstrap_ascending.requests_limit
        );
        assert_eq!(
            default_bootstrap_ascending.throttle_coefficient,
            deserialized_bootstrap_ascending.throttle_coefficient
        );
        assert_eq!(
            default_bootstrap_ascending.throttle_wait,
            deserialized_bootstrap_ascending.throttle_wait
        );
        assert_eq!(
            default_bootstrap_ascending.timeout,
            deserialized_bootstrap_ascending.timeout
        );

        let default_account_sets = default_bootstrap_ascending.account_sets.unwrap();
        let deserialized_account_sets = deserialized_bootstrap_ascending.account_sets.unwrap();

        assert_eq!(
            default_account_sets.blocking_max,
            deserialized_account_sets.blocking_max
        );
        assert_eq!(
            default_account_sets.consideration_count,
            deserialized_account_sets.consideration_count
        );
        assert_eq!(
            default_account_sets.cooldown,
            deserialized_account_sets.cooldown
        );
        assert_eq!(
            default_account_sets.priorities_max,
            deserialized_account_sets.priorities_max
        );

        let default_bootstrap_server = default_node.bootstrap_server.unwrap();
        let deserialized_bootstrap_server = deserialized_node.bootstrap_server.unwrap();

        assert_eq!(
            default_bootstrap_server.batch_size,
            deserialized_bootstrap_server.batch_size
        );
        assert_eq!(
            default_bootstrap_server.max_queue,
            deserialized_bootstrap_server.max_queue
        );
        assert_eq!(
            default_bootstrap_server.threads,
            deserialized_bootstrap_server.threads
        );

        let default_diagnostics_txn_tracking =
            default_node.diagnostics.unwrap().txn_tracking.unwrap();
        let deserialized_diagnostics_txn_tracking =
            deserialized_node.diagnostics.unwrap().txn_tracking.unwrap();

        assert_eq!(
            default_diagnostics_txn_tracking.enable,
            deserialized_diagnostics_txn_tracking.enable
        );
        assert_eq!(
            default_diagnostics_txn_tracking.ignore_writes_below_block_processor_max_time,
            deserialized_diagnostics_txn_tracking.ignore_writes_below_block_processor_max_time
        );
        assert_eq!(
            default_diagnostics_txn_tracking.min_read_txn_time,
            deserialized_diagnostics_txn_tracking.min_read_txn_time
        );
        assert_eq!(
            default_diagnostics_txn_tracking.min_write_txn_time,
            deserialized_diagnostics_txn_tracking.min_write_txn_time
        );

        let default_experimental = default_node.experimental.unwrap();
        let deserialized_experimental = deserialized_node.experimental.unwrap();

        assert_eq!(
            default_experimental.max_pruning_age,
            deserialized_experimental.max_pruning_age
        );
        assert_eq!(
            default_experimental.max_pruning_depth,
            deserialized_experimental.max_pruning_depth
        );
        assert_eq!(
            default_experimental.secondary_work_peers,
            deserialized_experimental.secondary_work_peers
        );

        let default_httpcallback = default_node.httpcallback.unwrap();
        let deserialized_httpcallback = deserialized_node.httpcallback.unwrap();

        assert_eq!(
            default_httpcallback.address,
            deserialized_httpcallback.address
        );
        assert_eq!(default_httpcallback.port, deserialized_httpcallback.port);
        assert_eq!(
            default_httpcallback.target,
            deserialized_httpcallback.target
        );

        let default_ipc_flatbuffers = default_node.ipc.clone().unwrap().flatbuffers.unwrap();
        let deserialized_ipc_flatbuffers =
            deserialized_node.ipc.clone().unwrap().flatbuffers.unwrap();

        assert_eq!(
            default_ipc_flatbuffers.skip_unexpected_fields_in_json,
            deserialized_ipc_flatbuffers.skip_unexpected_fields_in_json
        );
        assert_eq!(
            default_ipc_flatbuffers.verify_buffers,
            deserialized_ipc_flatbuffers.verify_buffers
        );

        let default_ipc_local = default_node.ipc.clone().unwrap().local.unwrap();
        let deserialized_ipc_local = deserialized_node.ipc.clone().unwrap().local.unwrap();

        assert_eq!(
            default_ipc_local.allow_unsafe,
            deserialized_ipc_local.allow_unsafe
        );
        assert_eq!(default_ipc_local.enable, deserialized_ipc_local.enable);
        assert_eq!(
            default_ipc_local.io_timeout,
            deserialized_ipc_local.io_timeout
        );
        assert_eq!(default_ipc_local.path, deserialized_ipc_local.path);

        let default_ipc_tcp = default_node.ipc.clone().unwrap().tcp.unwrap();
        let deserialized_ipc_tcp = deserialized_node.ipc.unwrap().tcp.unwrap();

        assert_eq!(
            default_node.ipc.clone().unwrap().tcp.unwrap().enable,
            deserialized_ipc_tcp.enable
        );
        assert_eq!(default_ipc_tcp.io_timeout, deserialized_ipc_tcp.io_timeout);
        assert_eq!(default_ipc_tcp.port, deserialized_ipc_tcp.port);

        let default_lmdb = default_node.lmdb.unwrap();
        let deserialized_lmdb = deserialized_node.lmdb.unwrap();

        assert_eq!(default_lmdb.map_size, deserialized_lmdb.map_size);
        assert_eq!(default_lmdb.max_databases, deserialized_lmdb.max_databases);
        assert_eq!(default_lmdb.sync, deserialized_lmdb.sync);

        let default_message_processor = default_node.message_processor.unwrap();
        let deserialized_message_processor = deserialized_node.message_processor.unwrap();

        assert_eq!(
            default_message_processor.max_queue,
            deserialized_message_processor.max_queue
        );
        assert_eq!(
            default_message_processor.threads,
            deserialized_message_processor.threads
        );

        let default_monitor = default_node.monitor.unwrap();
        let deserialized_monitor = deserialized_node.monitor.unwrap();

        assert_eq!(default_monitor.enable, deserialized_monitor.enable);
        assert_eq!(default_monitor.interval, deserialized_monitor.interval);

        let default_optimistic_scheduler = default_node.optimistic_scheduler.unwrap();
        let deserialized_optimistic_scheduler = deserialized_node.optimistic_scheduler.unwrap();

        assert_eq!(
            default_optimistic_scheduler.enable,
            deserialized_optimistic_scheduler.enable
        );
        assert_eq!(
            default_optimistic_scheduler.gap_threshold,
            deserialized_optimistic_scheduler.gap_threshold
        );
        assert_eq!(
            default_optimistic_scheduler.max_size,
            deserialized_optimistic_scheduler.max_size
        );

        let default_priority_bucket = default_node.priority_bucket.unwrap();
        let deserialized_priority_bucket = deserialized_node.priority_bucket.unwrap();

        assert_eq!(
            default_priority_bucket.max_blocks,
            deserialized_priority_bucket.max_blocks
        );
        assert_eq!(
            default_priority_bucket.max_elections,
            deserialized_priority_bucket.max_elections
        );
        assert_eq!(
            default_priority_bucket.reserved_elections,
            deserialized_priority_bucket.reserved_elections
        );

        let default_rep_crawler = default_node.rep_crawler.unwrap();
        let deserialized_rep_crawler = deserialized_node.rep_crawler.unwrap();

        assert_eq!(
            default_rep_crawler.query_timeout,
            deserialized_rep_crawler.query_timeout
        );

        let default_request_aggregator = default_node.request_aggregator.unwrap();
        let deserialized_request_aggregator = deserialized_node.request_aggregator.unwrap();

        assert_eq!(
            default_request_aggregator.batch_size,
            deserialized_request_aggregator.batch_size
        );
        assert_eq!(
            default_request_aggregator.max_queue,
            deserialized_request_aggregator.max_queue
        );
        assert_eq!(
            default_request_aggregator.threads,
            deserialized_request_aggregator.threads
        );

        let default_statistics = default_node.statistics.unwrap();
        let deserialized_statistics = deserialized_node.statistics.unwrap();

        assert_eq!(
            default_statistics.max_samples,
            deserialized_statistics.max_samples
        );

        let default_statistics_log = default_statistics.log.unwrap();
        let deserialized_statistics_log = deserialized_statistics.log.unwrap();

        assert_eq!(
            default_statistics_log.filename_counters,
            deserialized_statistics_log.filename_counters
        );

        let default_vote_cache = default_node.vote_cache.unwrap();
        let deserialized_vote_cache = deserialized_node.vote_cache.unwrap();

        assert_eq!(
            default_vote_cache.age_cutoff,
            deserialized_vote_cache.age_cutoff
        );
        assert_eq!(
            default_vote_cache.max_size,
            deserialized_vote_cache.max_size
        );
        assert_eq!(
            default_vote_cache.max_voters,
            deserialized_vote_cache.max_voters
        );

        let default_vote_processor = default_node.vote_processor.unwrap();
        let deserialized_vote_processor = deserialized_node.vote_processor.unwrap();

        assert_eq!(
            default_vote_processor.batch_size,
            deserialized_vote_processor.batch_size
        );
        assert_eq!(
            default_vote_processor.max_non_pr_queue,
            deserialized_vote_processor.max_non_pr_queue
        );
        assert_eq!(
            default_vote_processor.max_pr_queue,
            deserialized_vote_processor.max_pr_queue
        );
        assert_eq!(
            default_vote_processor.pr_priority,
            deserialized_vote_processor.pr_priority
        );
        assert_eq!(
            default_vote_processor.threads,
            deserialized_vote_processor.threads
        );

        let default_websocket = default_node.websocket.unwrap();
        let deserialized_websocket = deserialized_node.websocket.unwrap();

        assert_eq!(default_websocket.address, deserialized_websocket.address);
        assert_eq!(default_websocket.enable, deserialized_websocket.enable);
        assert_eq!(default_websocket.port, deserialized_websocket.port);

        let default_opencl = default_daemon_toml.opencl.unwrap();
        let deserialized_opencl = deserialized_toml.opencl.unwrap();

        assert_eq!(default_opencl.device, deserialized_opencl.device);
        assert_eq!(default_opencl.enable, deserialized_opencl.enable);
        assert_eq!(default_opencl.platform, deserialized_opencl.platform);
        assert_eq!(default_opencl.threads, deserialized_opencl.threads);

        let default_rpc = default_daemon_toml.rpc.unwrap();
        let deserialized_rpc = deserialized_toml.rpc.unwrap();

        assert_eq!(default_rpc.enable, deserialized_rpc.enable);
        assert_eq!(
            default_rpc.enable_sign_hash,
            deserialized_rpc.enable_sign_hash
        );

        let default_rpc_child_process = default_rpc.child_process.unwrap();
        let deserialized_rpc_child_process = deserialized_rpc.child_process.unwrap();

        assert_eq!(
            default_rpc_child_process.enable,
            deserialized_rpc_child_process.enable
        );
        assert_eq!(
            default_rpc_child_process.rpc_path,
            deserialized_rpc_child_process.rpc_path
        );
    }

    #[test]
    fn toml_deserialize_no_defaults() {
        let path: PathBuf = "node-config.toml".into();

        let toml_str = r#"
            [node]
           	allow_local_peers = false
           	backup_before_upgrade = true
           	bandwidth_limit = 999
           	bandwidth_limit_burst_ratio = 999.9
           	bootstrap_bandwidth_limit = 999
           	bootstrap_bandwidth_burst_ratio = 999.9
           	block_processor_batch_max_time = 999
           	bootstrap_connections = 999
           	bootstrap_connections_max = 999
           	bootstrap_initiator_threads = 999
           	bootstrap_serving_threads = 999
           	bootstrap_frontier_request_count = 9999
           	bootstrap_fraction_numerator = 999
           	confirming_set_batch_time = 999
           	enable_voting = false
           	external_address = "0:0:0:0:0:ffff:7f01:101"
           	external_port = 999
           	io_threads = 999
           	lmdb_max_dbs = 999
           	network_threads = 999
           	background_threads = 999
           	online_weight_minimum = "999"
           	representative_vote_weight_minimum = "999"
           	rep_crawler_weight_minimum = "999"
           	password_fanout = 999
           	peering_port = 999
           	pow_sleep_interval= 999
           	preconfigured_peers = ["dev.org"]
           	preconfigured_representatives = ["nano_3arg3asgtigae3xckabaaewkx3bzsh7nwz7jkmjos79ihyaxwphhm6qgjps4"]
           	receive_minimum = "999"
           	signature_checker_threads = 999
           	tcp_incoming_connections_max = 999
           	tcp_io_timeout = 999
           	unchecked_cutoff_time = 999
           	use_memory_pools = false
           	vote_generator_delay = 999
           	vote_generator_threshold = 9
           	vote_minimum = "999"
           	work_peers = ["dev.org:999"]
           	work_threads = 999
           	max_work_generate_multiplier = 1.0
           	max_queued_requests = 999
           	request_aggregator_threads = 999
           	max_unchecked_blocks = 999
           	frontiers_confirmation = "always"
           	backlog_scan_batch_size = 999
           	backlog_scan_frequency = 999

           	[node.block_processor]
           	max_peer_queue = 999
           	max_system_queue = 999
           	priority_live = 999
           	priority_bootstrap = 999
           	priority_local = 999

           	[node.active_elections]
           	size = 999
           	hinted_limit_percentage = 90
           	optimistic_limit_percentage = 90
           	confirmation_history_size = 999
           	confirmation_cache = 999

           	[node.diagnostics.txn_tracking]
           	enable = true
           	ignore_writes_below_block_processor_max_time = false
           	min_read_txn_time = 999
           	min_write_txn_time = 999

           	[node.httpcallback]
           	address = "dev.org"
           	port = 999
           	target = "/dev"

           	[node.ipc.local]
           	allow_unsafe = true
           	enable = true
           	io_timeout = 999
           	io_threads = 999
           	path = "/tmp/dev"

           	[node.ipc.tcp]
           	enable = true
           	io_timeout = 999
           	io_threads = 999
           	port = 999

           	[node.ipc.flatbuffers]
           	skip_unexpected_fields_in_json = false
           	verify_buffers = false

           	[node.statistics]
           	max_samples = 999

           	[node.statistics.log]
           	filename_counters = "devcounters.stat"
           	filename_samples = "devsamples.stat"
           	headers = false
           	interval_counters = 999
           	interval_samples = 999
           	rotation_count = 999

           	[node.websocket]
           	address = "0:0:0:0:0:ffff:7f01:101"
           	enable = true
           	port = 999

           	[node.lmdb]
           	sync = "nosync_safe"
           	max_databases = 999
           	map_size = 999

           	[node.optimistic_scheduler]
           	enable = false
           	gap_threshold = 999
           	max_size = 999

           	[node.hinted_scheduler]
           	enable = false
           	hinting_threshold = 99
           	check_interval = 999
           	block_cooldown = 999
           	vacancy_threshold = 99

           	[node.experimental]
           	secondary_work_peers = ["dev.org:998"]
           	max_pruning_age = 999
           	max_pruning_depth = 999

           	[node.vote_cache]
           	max_size = 999
           	max_voters = 999

           	[node.vote_processor]
           	max_pr_queue = 999
           	max_non_pr_queue = 999
           	pr_priority = 999
           	threads = 999
           	batch_size = 999

           	[node.bootstrap_server]
           	max_queue = 999
           	threads = 999
           	batch_size = 999

           	[node.request_aggregator]
           	max_queue = 999
           	threads = 999
           	batch_size = 999

           	[node.message_processor]
           	threads = 999
           	max_queue = 999

           	[opencl]
           	device = 999
           	enable = true
           	platform = 999
           	threads = 999

           	[rpc]
           	enable = true
           	enable_sign_hash = true

           	[rpc.child_process]
           	enable = true
           	rpc_path = "/dev/nano_rpc"
        "#;

        let fs = NullableFilesystem::null_builder()
            .read_to_string(&path, toml_str.to_string())
            .finish();

        let toml_read = fs.read_to_string(&path).unwrap();

        let daemon_toml: DaemonToml =
            toml::from_str(&toml_read).expect("Failed to deserialize TOML");

        let daemon_config: DaemonConfig = (&daemon_toml).into();

        let default_daemon_config = DaemonConfig::default();
    }

    #[test]
    fn deserialize_defaults() {
        let path: PathBuf = "node-config.toml".into();

        let toml_str = r#"
            [node]
            [node.block_processor]
            [node.diagnostics.txn_tracking]
            [node.httpcallback]
            [node.ipc.local]
            [node.ipc.tcp]
            [node.statistics.log]
            [node.statistics.sampling]
            [node.vote_processsor]
            [node.websocket]
            [node.lmdb]
            [node.bootstrap_server]
            [opencl]
            [rpc]
            [rpc.child_process]
        "#;

        let fs = NullableFilesystem::null_builder()
            .read_to_string(&path, toml_str.to_string())
            .finish();

        let toml_read = fs.read_to_string(&path).unwrap();

        let daemon_toml: DaemonToml =
            toml::from_str(&toml_read).expect("Failed to deserialize TOML");

        let daemon_toml: DaemonConfig = (&daemon_toml).into();

        let default_daemon_toml = DaemonConfig::default();

        let node_toml = daemon_toml.node;
        let default_node_toml = default_daemon_toml.node;

        assert_eq!(
            node_toml.allow_local_peers,
            default_node_toml.allow_local_peers
        );
        assert_eq!(
            node_toml.background_threads,
            default_node_toml.background_threads
        );
        assert_eq!(
            node_toml.backlog_scan_batch_size,
            default_node_toml.backlog_scan_batch_size
        );
        assert_eq!(
            node_toml.backlog_scan_frequency,
            default_node_toml.backlog_scan_frequency
        );
        assert_eq!(
            node_toml.backup_before_upgrade,
            default_node_toml.backup_before_upgrade
        );
        assert_eq!(node_toml.bandwidth_limit, default_node_toml.bandwidth_limit);
        assert_eq!(
            node_toml.bandwidth_limit_burst_ratio,
            default_node_toml.bandwidth_limit_burst_ratio
        );
        assert_eq!(
            node_toml.block_processor_batch_max_time_ms,
            default_node_toml.block_processor_batch_max_time_ms
        );
        assert_eq!(
            node_toml.bootstrap_bandwidth_burst_ratio,
            default_node_toml.bootstrap_bandwidth_burst_ratio
        );
        assert_eq!(
            node_toml.bootstrap_bandwidth_limit,
            default_node_toml.bootstrap_bandwidth_limit
        );
        assert_eq!(
            node_toml.bootstrap_connections,
            default_node_toml.bootstrap_connections
        );
        assert_eq!(
            node_toml.bootstrap_connections_max,
            default_node_toml.bootstrap_connections_max
        );
        assert_eq!(
            node_toml.bootstrap_fraction_numerator,
            default_node_toml.bootstrap_fraction_numerator
        );
        assert_eq!(
            node_toml.bootstrap_frontier_request_count,
            default_node_toml.bootstrap_frontier_request_count
        );
        assert_eq!(
            node_toml.bootstrap_initiator_threads,
            default_node_toml.bootstrap_initiator_threads
        );
        assert_eq!(
            node_toml.bootstrap_serving_threads,
            default_node_toml.bootstrap_serving_threads
        );
        assert_eq!(
            node_toml.confirming_set_batch_time,
            default_node_toml.confirming_set_batch_time
        );
        assert_eq!(node_toml.enable_voting, default_node_toml.enable_voting);
        assert_eq!(
            node_toml.external_address,
            default_node_toml.external_address
        );
        assert_eq!(node_toml.external_port, default_node_toml.external_port);
        assert_eq!(
            node_toml.frontiers_confirmation,
            default_node_toml.frontiers_confirmation
        );
        assert_eq!(node_toml.io_threads, default_node_toml.io_threads);
        assert_eq!(
            node_toml.max_queued_requests,
            default_node_toml.max_queued_requests
        );
        assert_eq!(
            node_toml.max_work_generate_multiplier,
            default_node_toml.max_work_generate_multiplier
        );
        assert_eq!(node_toml.network_threads, default_node_toml.network_threads);
        assert_eq!(
            node_toml.online_weight_minimum,
            default_node_toml.online_weight_minimum
        );
        assert_eq!(node_toml.password_fanout, default_node_toml.password_fanout);
        assert_eq!(node_toml.peering_port, default_node_toml.peering_port);
        assert_eq!(
            node_toml.pow_sleep_interval_ns,
            default_node_toml.pow_sleep_interval_ns
        );
        assert_eq!(
            node_toml.preconfigured_peers,
            default_node_toml.preconfigured_peers
        );
        assert_eq!(
            node_toml.preconfigured_representatives,
            default_node_toml.preconfigured_representatives
        );
        assert_eq!(node_toml.receive_minimum, default_node_toml.receive_minimum);
        assert_eq!(
            node_toml.rep_crawler_weight_minimum,
            default_node_toml.rep_crawler_weight_minimum
        );
        assert_eq!(
            node_toml.representative_vote_weight_minimum,
            default_node_toml.representative_vote_weight_minimum
        );
        assert_eq!(
            node_toml.request_aggregator_threads,
            default_node_toml.request_aggregator_threads
        );
        assert_eq!(
            node_toml.signature_checker_threads,
            default_node_toml.signature_checker_threads
        );
        assert_eq!(
            node_toml.tcp_incoming_connections_max,
            default_node_toml.tcp_incoming_connections_max
        );
        assert_eq!(
            node_toml.tcp_io_timeout_s,
            default_node_toml.tcp_io_timeout_s
        );
        assert_eq!(
            node_toml.unchecked_cutoff_time_s,
            default_node_toml.unchecked_cutoff_time_s
        );
        assert_eq!(
            node_toml.use_memory_pools,
            default_node_toml.use_memory_pools
        );
        assert_eq!(
            node_toml.vote_generator_delay_ms,
            default_node_toml.vote_generator_delay_ms
        );
        assert_eq!(
            node_toml.vote_generator_threshold,
            default_node_toml.vote_generator_threshold
        );
        assert_eq!(node_toml.vote_minimum, default_node_toml.vote_minimum);
        assert_eq!(node_toml.work_threads, default_node_toml.work_threads);

        let default_active_elections = node_toml.active_elections;
        let deserialized_active_elections = default_node_toml.active_elections;

        assert_eq!(
            default_active_elections.confirmation_cache,
            deserialized_active_elections.confirmation_cache
        );
        assert_eq!(
            default_active_elections.confirmation_history_size,
            deserialized_active_elections.confirmation_history_size
        );
        assert_eq!(
            default_active_elections.hinted_limit_percentage,
            deserialized_active_elections.hinted_limit_percentage
        );
        assert_eq!(
            default_active_elections.optimistic_limit_percentage,
            deserialized_active_elections.optimistic_limit_percentage
        );
        assert_eq!(
            default_active_elections.size,
            deserialized_active_elections.size
        );

        let default_block_processor = node_toml.block_processor;
        let deserialized_block_processor = default_node_toml.block_processor;

        assert_eq!(
            default_block_processor.max_peer_queue,
            deserialized_block_processor.max_peer_queue
        );
        assert_eq!(
            default_block_processor.max_system_queue,
            deserialized_block_processor.max_system_queue
        );
        assert_eq!(
            default_block_processor.priority_bootstrap,
            deserialized_block_processor.priority_bootstrap
        );
        assert_eq!(
            default_block_processor.priority_live,
            deserialized_block_processor.priority_live
        );
        assert_eq!(
            default_block_processor.priority_local,
            deserialized_block_processor.priority_local
        );

        let default_bootstrap_ascending = node_toml.bootstrap_ascending;
        let deserialized_bootstrap_ascending = default_node_toml.bootstrap_ascending;

        assert_eq!(
            default_bootstrap_ascending.block_wait_count,
            deserialized_bootstrap_ascending.block_wait_count
        );
        assert_eq!(
            default_bootstrap_ascending.database_requests_limit,
            deserialized_bootstrap_ascending.database_requests_limit
        );
        assert_eq!(
            default_bootstrap_ascending.pull_count,
            deserialized_bootstrap_ascending.pull_count
        );
        assert_eq!(
            default_bootstrap_ascending.requests_limit,
            deserialized_bootstrap_ascending.requests_limit
        );
        assert_eq!(
            default_bootstrap_ascending.throttle_coefficient,
            deserialized_bootstrap_ascending.throttle_coefficient
        );
        assert_eq!(
            default_bootstrap_ascending.throttle_wait,
            deserialized_bootstrap_ascending.throttle_wait
        );
        assert_eq!(
            default_bootstrap_ascending.timeout,
            deserialized_bootstrap_ascending.timeout
        );

        let default_account_sets = default_bootstrap_ascending.account_sets;
        let deserialized_account_sets = deserialized_bootstrap_ascending.account_sets;

        assert_eq!(
            default_account_sets.blocking_max,
            deserialized_account_sets.blocking_max
        );
        assert_eq!(
            default_account_sets.consideration_count,
            deserialized_account_sets.consideration_count
        );
        assert_eq!(
            default_account_sets.cooldown,
            deserialized_account_sets.cooldown
        );
        assert_eq!(
            default_account_sets.priorities_max,
            deserialized_account_sets.priorities_max
        );

        let default_bootstrap_server = node_toml.bootstrap_server;
        let deserialized_bootstrap_server = default_node_toml.bootstrap_server;

        assert_eq!(
            default_bootstrap_server.batch_size,
            deserialized_bootstrap_server.batch_size
        );
        assert_eq!(
            default_bootstrap_server.max_queue,
            deserialized_bootstrap_server.max_queue
        );
        assert_eq!(
            default_bootstrap_server.threads,
            deserialized_bootstrap_server.threads
        );

        let default_diagnostics_txn_tracking = node_toml.diagnostics_config.txn_tracking;
        let deserialized_diagnostics_txn_tracking =
            default_node_toml.diagnostics_config.txn_tracking;

        assert_eq!(
            default_diagnostics_txn_tracking.enable,
            deserialized_diagnostics_txn_tracking.enable
        );
        assert_eq!(
            default_diagnostics_txn_tracking.ignore_writes_below_block_processor_max_time,
            deserialized_diagnostics_txn_tracking.ignore_writes_below_block_processor_max_time
        );
        assert_eq!(
            default_diagnostics_txn_tracking.min_read_txn_time_ms,
            deserialized_diagnostics_txn_tracking.min_read_txn_time_ms
        );
        assert_eq!(
            default_diagnostics_txn_tracking.min_write_txn_time_ms,
            deserialized_diagnostics_txn_tracking.min_write_txn_time_ms
        );

        assert_eq!(
            default_node_toml.callback_address,
            node_toml.callback_address
        );
        assert_eq!(default_node_toml.callback_port, node_toml.callback_port);
        assert_eq!(default_node_toml.callback_target, node_toml.callback_target);

        let default_ipc_flatbuffers = node_toml.ipc_config.flatbuffers;
        let deserialized_ipc_flatbuffers = default_node_toml.ipc_config.flatbuffers;

        assert_eq!(
            default_ipc_flatbuffers.skip_unexpected_fields_in_json,
            deserialized_ipc_flatbuffers.skip_unexpected_fields_in_json
        );
        assert_eq!(
            default_ipc_flatbuffers.verify_buffers,
            deserialized_ipc_flatbuffers.verify_buffers
        );

        let default_ipc_local = node_toml.ipc_config.transport_domain;
        let deserialized_ipc_local = default_node_toml.ipc_config.transport_domain;

        assert_eq!(
            default_ipc_local.transport.allow_unsafe,
            deserialized_ipc_local.transport.allow_unsafe
        );
        assert_eq!(
            default_ipc_local.transport.enabled,
            deserialized_ipc_local.transport.enabled
        );
        assert_eq!(
            default_ipc_local.transport.io_timeout,
            deserialized_ipc_local.transport.io_timeout
        );
        assert_eq!(default_ipc_local.path, deserialized_ipc_local.path);

        let default_ipc_tcp = node_toml.ipc_config.transport_tcp;
        let deserialized_ipc_tcp = default_node_toml.ipc_config.transport_tcp;

        assert_eq!(
            default_ipc_tcp.transport.enabled,
            deserialized_ipc_tcp.transport.enabled
        );
        assert_eq!(
            default_ipc_tcp.transport.io_timeout,
            deserialized_ipc_tcp.transport.io_timeout
        );
        assert_eq!(default_ipc_tcp.port, deserialized_ipc_tcp.port);

        let default_lmdb = node_toml.lmdb_config;
        let deserialized_lmdb = default_node_toml.lmdb_config;

        assert_eq!(default_lmdb.map_size, deserialized_lmdb.map_size);
        assert_eq!(default_lmdb.max_databases, deserialized_lmdb.max_databases);
        assert_eq!(default_lmdb.sync, deserialized_lmdb.sync);

        let default_message_processor = node_toml.message_processor;
        let deserialized_message_processor = default_node_toml.message_processor;

        assert_eq!(
            default_message_processor.max_queue,
            deserialized_message_processor.max_queue
        );
        assert_eq!(
            default_message_processor.threads,
            deserialized_message_processor.threads
        );

        let default_monitor = node_toml.monitor;
        let deserialized_monitor = default_node_toml.monitor;

        assert_eq!(default_monitor.enabled, deserialized_monitor.enabled);
        assert_eq!(default_monitor.interval, deserialized_monitor.interval);

        let default_optimistic_scheduler = node_toml.optimistic_scheduler;
        let deserialized_optimistic_scheduler = default_node_toml.optimistic_scheduler;

        assert_eq!(
            default_optimistic_scheduler.enabled,
            deserialized_optimistic_scheduler.enabled
        );
        assert_eq!(
            default_optimistic_scheduler.gap_threshold,
            deserialized_optimistic_scheduler.gap_threshold
        );
        assert_eq!(
            default_optimistic_scheduler.max_size,
            deserialized_optimistic_scheduler.max_size
        );

        let default_priority_bucket = node_toml.priority_bucket;
        let deserialized_priority_bucket = default_node_toml.priority_bucket;

        assert_eq!(
            default_priority_bucket.max_blocks,
            deserialized_priority_bucket.max_blocks
        );
        assert_eq!(
            default_priority_bucket.max_elections,
            deserialized_priority_bucket.max_elections
        );
        assert_eq!(
            default_priority_bucket.reserved_elections,
            deserialized_priority_bucket.reserved_elections
        );

        assert_eq!(
            node_toml.rep_crawler_query_timeout,
            default_node_toml.rep_crawler_query_timeout
        );

        let default_request_aggregator = node_toml.request_aggregator;
        let deserialized_request_aggregator = default_node_toml.request_aggregator;

        assert_eq!(
            default_request_aggregator.batch_size,
            deserialized_request_aggregator.batch_size
        );
        assert_eq!(
            default_request_aggregator.max_queue,
            deserialized_request_aggregator.max_queue
        );
        assert_eq!(
            default_request_aggregator.threads,
            deserialized_request_aggregator.threads
        );

        let default_statistics = node_toml.stat_config;
        let deserialized_statistics = default_node_toml.stat_config;

        assert_eq!(
            default_statistics.max_samples,
            deserialized_statistics.max_samples
        );

        assert_eq!(
            default_statistics.log_counters_filename,
            deserialized_statistics.log_counters_filename
        );

        assert_eq!(
            default_statistics.log_samples_filename,
            deserialized_statistics.log_samples_filename
        );

        let default_vote_cache = node_toml.vote_cache;
        let deserialized_vote_cache = default_node_toml.vote_cache;

        assert_eq!(
            default_vote_cache.age_cutoff,
            deserialized_vote_cache.age_cutoff
        );
        assert_eq!(
            default_vote_cache.max_size,
            deserialized_vote_cache.max_size
        );
        assert_eq!(
            default_vote_cache.max_voters,
            deserialized_vote_cache.max_voters
        );

        let default_vote_processor = node_toml.vote_processor;
        let deserialized_vote_processor = default_node_toml.vote_processor;

        assert_eq!(
            default_vote_processor.batch_size,
            deserialized_vote_processor.batch_size
        );
        assert_eq!(
            default_vote_processor.max_non_pr_queue,
            deserialized_vote_processor.max_non_pr_queue
        );
        assert_eq!(
            default_vote_processor.max_pr_queue,
            deserialized_vote_processor.max_pr_queue
        );
        assert_eq!(
            default_vote_processor.pr_priority,
            deserialized_vote_processor.pr_priority
        );
        assert_eq!(
            default_vote_processor.threads,
            deserialized_vote_processor.threads
        );

        let default_websocket = node_toml.websocket_config;
        let deserialized_websocket = default_node_toml.websocket_config;

        assert_eq!(default_websocket.address, deserialized_websocket.address);
        assert_eq!(default_websocket.enabled, deserialized_websocket.enabled);
        assert_eq!(default_websocket.port, deserialized_websocket.port);

        let default_opencl = default_daemon_toml.opencl;
        let deserialized_opencl = daemon_toml.opencl;

        assert_eq!(default_opencl.device, deserialized_opencl.device);
        assert_eq!(default_daemon_toml.opencl_enable, daemon_toml.opencl_enable);
        assert_eq!(default_opencl.platform, deserialized_opencl.platform);
        assert_eq!(default_opencl.threads, deserialized_opencl.threads);

        let default_rpc = default_daemon_toml.rpc;
        let deserialized_rpc = daemon_toml.rpc;

        assert_eq!(default_daemon_toml.rpc_enable, daemon_toml.rpc_enable);
        assert_eq!(
            default_rpc.enable_sign_hash,
            deserialized_rpc.enable_sign_hash
        );

        let default_rpc_child_process = default_rpc.child_process;
        let deserialized_rpc_child_process = deserialized_rpc.child_process;

        assert_eq!(
            default_rpc_child_process.enable,
            deserialized_rpc_child_process.enable
        );
        assert_eq!(
            default_rpc_child_process.rpc_path,
            deserialized_rpc_child_process.rpc_path
        );
    }
}
