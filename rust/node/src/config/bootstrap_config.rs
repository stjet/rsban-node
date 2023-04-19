use std::time::Duration;

use rsnano_core::utils::TomlWriter;

use crate::messages::BlocksAckPayload;

pub struct BootstrapAscendingConfig {
    /// Maximum number of un-responded requests per channel
    pub requests_limit: usize,
    pub database_requests_limit: usize,
    pub pull_count: usize,
    pub timeout: Duration,
    pub throttle_count: usize,
    pub throttle_wait: Duration,
    pub account_sets: AccountSetsConfig,
}

impl BootstrapAscendingConfig {
    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize ("requests_limit", self.requests_limit, "Request limit to ascending bootstrap after which requests will be dropped.\nNote: changing to unlimited (0) is not recommended.\ntype:uint64")?;
        toml.put_usize ("database_requests_limit", self.database_requests_limit, "Request limit for accounts from database after which requests will be dropped.\nNote: changing to unlimited (0) is not recommended as this operation competes for resources on querying the database.\ntype:uint64")?;
        toml.put_usize(
            "pull_count",
            self.pull_count,
            "Number of requested blocks for ascending bootstrap request.\ntype:uint64",
        )?;
        toml.put_u64 ("timeout", self.timeout.as_millis() as u64, "Timeout in milliseconds for incoming ascending bootstrap messages to be processed.\ntype:milliseconds")?;
        toml.put_usize(
            "throttle_count",
            self.throttle_count,
            "Number of samples to track for bootstrap throttling.\ntype:uint64",
        )?;
        toml.put_u64(
            "throttle_wait",
            self.throttle_wait.as_millis() as u64,
            "Length of time to wait between requests when throttled.\ntype:milliseconds",
        )?;

        toml.put_child("account_sets", &mut |child| {
            self.account_sets.serialize_toml(child)
        })
    }
}

impl Default for BootstrapAscendingConfig {
    fn default() -> Self {
        Self {
            requests_limit: 4,
            database_requests_limit: 1024,
            pull_count: BlocksAckPayload::MAX_BLOCKS,
            timeout: Duration::from_secs(3),
            throttle_count: 4 * 1024,
            throttle_wait: Duration::from_millis(100),
            account_sets: Default::default(),
        }
    }
}

pub struct AccountSetsConfig {
    pub consideration_count: usize,
    pub priorities_max: usize,
    pub blocking_max: usize,
    pub cooldown: Duration,
}

impl AccountSetsConfig {
    pub(crate) fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize ("consideration_count", self.consideration_count, "Limit the number of account candidates to consider and also the number of iterations.\ntype:uint64")?;
        toml.put_usize(
            "priorities_max",
            self.priorities_max,
            "Cutoff size limit for the priority list.\ntype:uint64",
        )?;
        toml.put_usize(
            "blocking_max",
            self.blocking_max,
            "Cutoff size limit for the blocked accounts from the priority list.\ntype:uint64",
        )?;
        toml.put_u64(
            "cooldown",
            self.cooldown.as_millis() as u64,
            "Waiting time for an account to become available.\ntype:milliseconds",
        )
    }
}

impl Default for AccountSetsConfig {
    fn default() -> Self {
        Self {
            consideration_count: 4,
            priorities_max: 256 * 1024,
            blocking_max: 256 * 1024,
            cooldown: Duration::from_secs(3),
        }
    }
}
