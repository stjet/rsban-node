use crate::{bootstrap::BootstrapAscendingConfig, config::BootstrapAscendingToml};
use std::time::Duration;

impl From<&BootstrapAscendingConfig> for BootstrapAscendingToml {
    fn from(config: &BootstrapAscendingConfig) -> Self {
        Self {
            requests_limit: Some(config.requests_limit),
            database_requests_limit: Some(config.database_requests_limit),
            pull_count: Some(config.pull_count),
            timeout: Some(config.timeout.as_millis() as u64),
            throttle_coefficient: Some(config.throttle_coefficient),
            throttle_wait: Some(config.throttle_wait.as_millis() as u64),
            account_sets: Some((&config.account_sets).into()),
            block_wait_count: Some(config.block_wait_count),
        }
    }
}

impl From<&BootstrapAscendingToml> for BootstrapAscendingConfig {
    fn from(toml: &BootstrapAscendingToml) -> Self {
        let mut config = BootstrapAscendingConfig::default();

        if let Some(account_sets) = &toml.account_sets {
            config.account_sets = account_sets.into();
        }
        if let Some(block_wait_count) = toml.block_wait_count {
            config.block_wait_count = block_wait_count;
        }
        if let Some(database_requests_limit) = toml.database_requests_limit {
            config.database_requests_limit = database_requests_limit;
        }
        if let Some(pull_count) = toml.pull_count {
            config.pull_count = pull_count;
        }
        if let Some(requests_limit) = toml.requests_limit {
            config.requests_limit = requests_limit;
        }
        if let Some(timeout) = &toml.timeout {
            config.timeout = Duration::from_millis(*timeout);
        }
        if let Some(throttle_wait) = &toml.throttle_wait {
            config.throttle_wait = Duration::from_millis(*throttle_wait);
        }
        if let Some(throttle_coefficient) = toml.throttle_coefficient {
            config.throttle_coefficient = throttle_coefficient;
        }
        config
    }
}
