use crate::bootstrap::{AccountSetsConfig, BootstrapAscendingConfig};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct BootstrapAscendingToml {
    pub block_wait_count: Option<usize>,
    pub database_requests_limit: Option<usize>,
    pub pull_count: Option<usize>,
    pub requests_limit: Option<usize>,
    pub throttle_coefficient: Option<usize>,
    pub throttle_wait: Option<u64>,
    pub timeout: Option<u64>,
    pub account_sets: Option<AccountSetsToml>,
}

impl Default for BootstrapAscendingToml {
    fn default() -> Self {
        let config = BootstrapAscendingConfig::default();
        (&config).into()
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AccountSetsToml {
    pub blocking_max: Option<usize>,
    pub consideration_count: Option<usize>,
    pub cooldown: Option<u64>,
    pub priorities_max: Option<usize>,
}

impl Default for AccountSetsToml {
    fn default() -> Self {
        let config = AccountSetsConfig::default();
        Self {
            consideration_count: Some(config.consideration_count),
            priorities_max: Some(config.priorities_max),
            blocking_max: Some(config.blocking_max),
            cooldown: Some(config.cooldown.as_millis() as u64),
        }
    }
}
