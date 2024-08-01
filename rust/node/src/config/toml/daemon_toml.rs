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
            rpc: Some(NodeRpcToml::new()),
        }
    }
}

impl DaemonToml {
    pub fn merge_defaults(&self, default_config: &DaemonToml) -> Result<String> {
        let defaults_str = toml::to_string(default_config)?;
        let current_str = toml::to_string(self)?;

        let mut result = String::new();
        let mut stream_defaults = defaults_str.lines().peekable();
        let mut stream_current = current_str.lines().peekable();

        while stream_current.peek().is_some() || stream_defaults.peek().is_some() {
            match (stream_defaults.peek(), stream_current.peek()) {
                (Some(&line_defaults), Some(&line_current)) => {
                    if line_defaults == line_current {
                        result.push_str(line_defaults);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    } else if line_current.starts_with('#') {
                        result.push_str("# ");
                        result.push_str(line_defaults);
                        result.push('\n');

                        result.push_str(line_current);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    } else {
                        result.push_str("# ");
                        result.push_str(line_defaults);
                        result.push('\n');
                        result.push_str(line_current);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    }
                }
                (Some(&line_defaults), None) => {
                    result.push_str("# ");
                    result.push_str(line_defaults);
                    result.push('\n');
                    stream_defaults.next();
                }
                (None, Some(&line_current)) => {
                    result.push_str(line_current);
                    result.push('\n');
                    stream_current.next();
                }
                _ => {}
            }
        }

        Ok(result)
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
    fn toml_serialize() {
        let network_params = NetworkParams::new(NetworkConstants::active_network());
        let config: DaemonToml = (&DaemonConfig::new(&network_params, 0)).into();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized_config: DaemonToml = toml::from_str(&toml_str).unwrap();

        assert_eq!(
            serde_json::to_string(&config).unwrap(),
            serde_json::to_string(&deserialized_config).unwrap()
        );
    }

    #[test]
    fn toml_deserialize_no_defaults() {
        let path: PathBuf = "/tmp/".into();

        let fs = NullableFilesystem::new_null();

        fs.create_dir_all(&path).unwrap();

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

        let file_path: PathBuf = path.join("config-node.toml");

        fs.write(&file_path, toml_str).unwrap();

        let path: PathBuf = "/tmp/config-node.toml".into();
        std::fs::write(&path, toml_str).unwrap();

        let toml_read = NullableFilesystem::new().read_to_string(&path).unwrap();

        let daemon_toml: DaemonToml =
            toml::from_str(&toml_read).expect("Failed to deserialize TOML");

        let daemon_config: DaemonConfig = (&daemon_toml).into();

        let default_daemon_config = DaemonConfig::default();

        assert_ne!(daemon_config, default_daemon_config);
    }

    #[test]
    fn deserialize_defaults() {
        let path: PathBuf = "/tmp/".into();

        let fs = NullableFilesystem::new_null();

        fs.create_dir_all(&path).unwrap();

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

        let file_path: PathBuf = path.join("config-node.toml");

        fs.write(&file_path, toml_str).unwrap();

        let path: PathBuf = "/tmp/config-node.toml".into();
        std::fs::write(&path, toml_str).unwrap();

        let toml_read = NullableFilesystem::new().read_to_string(&path).unwrap();

        let daemon_toml: DaemonToml =
            toml::from_str(&toml_read).expect("Failed to deserialize TOML");

        let daemon_config: DaemonConfig = (&daemon_toml).into();

        let default_daemon_config = DaemonConfig::default();

        assert_eq!(daemon_config, default_daemon_config);
    }
}
