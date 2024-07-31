use super::{NodeRpcToml, NodeToml, OpenclToml};
use crate::config::DaemonConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct DaemonToml {
    pub node: Option<NodeToml>,
    pub rpc: Option<NodeRpcToml>,
    pub opencl: Option<OpenclToml>,
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

impl DaemonToml {
    pub fn default() -> Result<Self> {
        Ok(Self {
            node: Some(NodeToml::default()),
            opencl: Some(OpenclToml::default()),
            rpc: Some(NodeRpcToml::new()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        config::{DaemonConfig, DaemonToml, NetworkConstants},
        nullable_fs::NullableFilesystem,
        NetworkParams,
    };

    #[test]
    fn test_toml_serialization() {
        let network_params = NetworkParams::new(NetworkConstants::active_network());
        let config: DaemonToml = (&DaemonConfig::new(&network_params, 0).unwrap()).into();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized_config: DaemonToml = toml::from_str(&toml_str).unwrap();

        assert_eq!(
            serde_json::to_string(&config).unwrap(),
            serde_json::to_string(&deserialized_config).unwrap()
        );
    }

    #[test]
    fn test_toml_deserialization() {
        let path: PathBuf = "/tmp/".into();

        let fs = NullableFilesystem::new_null();

        fs.create_dir_all(&path).unwrap();

        let toml_write = r#"
            [node]
           	# allow_local_peers = true
           	background_threads = 10
           	# backlog_scan_batch_size = 10000
           	# backlog_scan_frequency = 10
           	# backup_before_upgrade = false
           	# bandwidth_limit = 10485760
           	# bandwidth_limit_burst_ratio = 3.0
           	# block_processor_batch_max_time_ms = 500
           	# bootstrap_bandwidth_burst_ratio = 1.0
           	# bootstrap_bandwidth_limit = 5242880
           	# bootstrap_connections = 4
           	# bootstrap_connections_max = 64
           	# bootstrap_fraction_numerator = 1
           	# bootstrap_frontier_request_count = 1048576
           	# bootstrap_initiator_threads = 1
           	# bootstrap_serving_threads = 1
           	# confirming_set_batch_time = "250"
           	# enable_voting = false
           	# external_address = "::"
           	# external_port = 0
           	# frontiers_confirmation = "Automatic"
           	# io_threads = 4
           	# max_queued_requests = 512
           	# max_unchecked_blocks = 65536
           	# max_work_generate_multiplier = 64.0
           	# network_threads = 4
           	# online_weight_minimum = "60000000000000000000000000000000000000"
           	# password_fanout = 1024
           	# pow_sleep_interval_ns = 0
           	# preconfigured_peers = ["peering-beta.nano.org"]
           	# preconfigured_representatives = ["nano_1defau1t9off1ine9rep99999999999999999999999999999999wgmuzxxy"]
           	# receive_minimum = "1000000000000000000000000"
           	# rep_crawler_weight_minimum = "340282366920938463463374607431768211455"
           	# representative_vote_weight_minimum = "10000000000000000000000000000000"
           	# request_aggregator_threads = 4
           	# signature_checker_threads = 0
           	# tcp_incoming_connections_max = 2048
           	# tcp_io_timeout_s = 15
           	# unchecked_cutoff_time_s = 14400
           	# use_memory_pools = true
           	# vote_generator_delay_ms = 100
           	# vote_generator_threshold = 3
           	# vote_minimum = "1000000000000000000000000000000000"
           	# work_peers = []
           	# work_threads = 4
           	# secondary_work_peers = ["127.0.0.1:8076"]
           	# max_pruning_age_s = 300
           	# max_pruning_depth = 0
           	# callback_address = ""
           	# callback_port = 0
           	# callback_target = ""

            [node.optimistic_scheduler]
           	# enabled = true
           	# gap_threshold = 32
           	# max_size = 65536

            [node.priority_bucket]
           	# max_blocks = 8192
           	# reserved_elections = 150
           	# max_elections = 100

            [node.bootstrap_ascending]
           	# requests_limit = 64
           	# database_requests_limit = 1024
           	# pull_count = 128
           	# timeout = "3000"
           	# throttle_coefficient = 16
           	# throttle_wait = "100"
           	# block_wait_count = 1000

            [node.bootstrap_ascending.account_sets]
           	# consideration_count = 4
           	# priorities_max = 262144
           	# blocking_max = 262144
           	# cooldown = "3000"

            [node.bootstrap_server]
            # max_queue = 16
            # threads = 1
            # batch_size = 64

            [node.toml_websocket_config]
            # enabled = false
            # port = 57000
            # address = "::1"

            [node.ipc_config.transport_domain]
            # path = "/tmp/nano"

            [node.ipc_config.transport_domain.transport]
           	# enabled = false
           	# io_timeout = 15

            [node.ipc_config.transport_tcp]
           	# port = 56000

            [node.ipc_config.transport_tcp.transport]
           	# enabled = false
           	# io_timeout = 15

            [node.ipc_config.flatbuffers]
           	# skip_unexpected_fields_in_json = true
           	# verify_buffers = true

            [node.diagnostics_config.txn_tracking]
           	# enable = false
           	# min_read_txn_time_ms = 5000
           	# min_write_txn_time_ms = 500
           	# ignore_writes_below_block_processor_max_time = true

            [node.stat_config]
           	# max_samples = 16384
           	# log_samples_interval = "0"
           	# log_counters_interval = "0"
           	# log_rotation_count = 100
           	# log_headers = true
           	# log_counters_filename = "counters.stat"
           	# log_samples_filename = "samples.stat"

            [node.lmdb_config]
           	# sync = "Always"
           	# max_databases = 128
           	# map_size = 274877906944

            [node.vote_cache]
           	# max_size = 65536
           	# max_voters = 64
           	# age_cutoff = "900000"

            [node.block_processor]
           	# max_peer_queue = 128
           	# max_system_queue = 16384
           	# priority_live = 1
           	# priority_bootstrap = 8
           	# priority_local = 16

            [node.active_elections]
           	# size = 5000
           	# hinted_limit_percentage = 20
           	# optimistic_limit_percentage = 10
           	# confirmation_history_size = 2048
           	# confirmation_cache = 65536

            [node.vote_processor]
           	# max_pr_queue = 32
           	# max_non_pr_queue = 32
           	# pr_priority = 3
           	# threads = 1
           	# batch_size = 1024
           	# max_triggered = 16384

            [node.request_aggregator]
           	# threads = 1
           	# max_queue = 128
           	# batch_size = 16

            [node.message_processor]
           	# threads = 1
           	# max_queue = 64

            [node.monitor]
           	# enabled = true
           	# interval = 60

            [rpc]
           	# enable = false
           	# enable_sign_hash = false

            [rpc.child_process]
           	# enable = false
           	# rpc_path = "/Users/ruimorais/rsnano/rust/../build/cargo/debug/nano_rpc"

            [opencl]
           	# platform = 0
           	# device = 0
           	# threads = 1048576
        "#;

        let file_path: PathBuf = path.join("config-node.toml");

        fs.write(&file_path, toml_write).unwrap();

        let path: PathBuf = "/tmp/config-node.toml".into();
        std::fs::write(&path, toml_write).unwrap();

        let toml_read = NullableFilesystem::new().read_to_string(&path).unwrap();

        let toml_config: DaemonToml =
            toml::from_str(&toml_read).expect("Failed to deserialize TOML");

        assert_eq!(toml_config.node.unwrap().background_threads.unwrap(), 10);
    }
}
