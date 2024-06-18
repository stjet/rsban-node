use std::time::Duration;

use super::GlobalConfig;
use crate::{block_processing::BlockProcessorConfig, bootstrap::BootstrapAscendingConfig};

impl From<&GlobalConfig> for BlockProcessorConfig {
    fn from(value: &GlobalConfig) -> Self {
        let config = &value.node_config.block_processor;
        Self {
            max_peer_queue: config.max_peer_queue,
            priority_local: config.priority_local,
            priority_bootstrap: config.priority_bootstrap,
            priority_live: config.priority_live,
            max_system_queue: config.max_system_queue,
            batch_max_time: Duration::from_millis(
                value.node_config.block_processor_batch_max_time_ms as u64,
            ),
            full_size: value.flags.block_processor_full_size,
            batch_size: value.flags.block_processor_batch_size,
            work_thresholds: value.network_params.work.clone(),
        }
    }
}

impl From<&GlobalConfig> for BootstrapAscendingConfig {
    fn from(value: &GlobalConfig) -> Self {
        let config = &value.node_config.bootstrap_ascending;
        Self {
            requests_limit: config.requests_limit,
            database_requests_limit: config.database_requests_limit,
            pull_count: config.pull_count,
            timeout: config.timeout,
            throttle_coefficient: config.throttle_coefficient,
            throttle_wait: config.throttle_wait,
            account_sets: config.account_sets.clone(),
            block_wait_count: config.block_wait_count,
        }
    }
}
