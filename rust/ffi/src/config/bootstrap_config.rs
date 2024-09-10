use rsnano_node::bootstrap::{AccountSetsConfig, BootstrapAscendingConfig};
use std::time::Duration;

#[repr(C)]
pub struct BootstrapAscendingConfigDto {
    pub enable: bool,
    pub enable_database_scan: bool,
    pub enable_dependency_walker: bool,
    pub channel_limit: usize,
    pub database_rate_limit: usize,
    pub database_warmup_ratio: usize,
    pub max_pull_count: usize,
    pub timeout_ms: u64,
    pub throttle_coefficient: usize,
    pub throttle_wait_ms: u64,
    pub account_sets: AccountSetsConfigDto,
    pub block_processor_threshold: usize,
    pub max_requests: usize,
}

#[repr(C)]
pub struct AccountSetsConfigDto {
    pub consideration_count: usize,
    pub priorities_max: usize,
    pub blocking_max: usize,
    pub cooldown_ms: u64,
}

impl From<&BootstrapAscendingConfig> for BootstrapAscendingConfigDto {
    fn from(value: &BootstrapAscendingConfig) -> Self {
        Self {
            enable: value.enable,
            enable_database_scan: value.enable_database_scan,
            enable_dependency_walker: value.enable_dependency_walker,
            channel_limit: value.channel_limit,
            database_rate_limit: value.database_rate_limit,
            database_warmup_ratio: value.database_warmup_ratio,
            max_pull_count: value.max_pull_count,
            timeout_ms: value.request_timeout.as_millis() as u64,
            throttle_coefficient: value.throttle_coefficient,
            throttle_wait_ms: value.throttle_wait.as_millis() as u64,
            account_sets: (&value.account_sets).into(),
            block_processor_threshold: value.block_processor_theshold,
            max_requests: value.max_requests,
        }
    }
}

impl From<&BootstrapAscendingConfigDto> for BootstrapAscendingConfig {
    fn from(value: &BootstrapAscendingConfigDto) -> Self {
        let mut config = BootstrapAscendingConfig::default();
        config.enable = value.enable;
        config.enable_database_scan = value.enable_database_scan;
        config.enable_dependency_walker = value.enable_dependency_walker;
        config.channel_limit = value.channel_limit;
        config.database_rate_limit = value.database_rate_limit;
        config.database_warmup_ratio = value.database_warmup_ratio;
        config.max_pull_count = value.max_pull_count;
        config.request_timeout = Duration::from_millis(value.timeout_ms);
        config.throttle_coefficient = value.throttle_coefficient;
        config.throttle_wait = Duration::from_millis(value.throttle_wait_ms);
        config.account_sets = (&value.account_sets).into();
        config.block_processor_theshold = value.block_processor_threshold;
        config.max_requests = value.max_requests;
        config
    }
}

impl From<&AccountSetsConfig> for AccountSetsConfigDto {
    fn from(value: &AccountSetsConfig) -> Self {
        Self {
            consideration_count: value.consideration_count,
            priorities_max: value.priorities_max,
            blocking_max: value.blocking_max,
            cooldown_ms: value.cooldown.as_millis() as u64,
        }
    }
}

impl From<&AccountSetsConfigDto> for AccountSetsConfig {
    fn from(value: &AccountSetsConfigDto) -> Self {
        Self {
            consideration_count: value.consideration_count,
            priorities_max: value.priorities_max,
            blocking_max: value.blocking_max,
            cooldown: Duration::from_millis(value.cooldown_ms),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_account_sets_config_create(result: *mut AccountSetsConfigDto) {
    (*result) = (&AccountSetsConfig::default()).into()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_config_create(result: *mut BootstrapAscendingConfigDto) {
    (*result) = (&BootstrapAscendingConfig::default()).into()
}
