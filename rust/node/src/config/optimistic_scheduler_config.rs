use std::time::Duration;

use rsnano_core::utils::TomlWriter;

pub struct OptimisticSchedulerConfig {
    pub enabled: bool,

    /// Minimum difference between confirmation frontier and account frontier to become a candidate for optimistic confirmation
    pub gap_threshold: usize,

    /// Maximum number of candidates stored in memory
    pub max_size: usize,
}

impl OptimisticSchedulerConfig {
    pub fn new() -> Self {
        Self {
            enabled: true,
            gap_threshold: 32,
            max_size: 1024 * 64,
        }
    }

    pub(crate) fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_bool(
            "enable",
            self.enabled,
            "Enable or disable optimistic elections\ntype:bool",
        )?;
        toml.put_usize ("gap_threshold", self.gap_threshold, "Minimum difference between confirmation frontier and account frontier to become a candidate for optimistic confirmation\ntype:uint64")?;
        toml.put_usize(
            "max_size",
            self.max_size,
            "Maximum number of candidates stored in memory\ntype:uint64",
        )
    }
}

pub struct HintedSchedulerConfig {
    pub check_interval: Duration,
    pub block_cooldown: Duration,
    pub hinting_theshold_percent: u32,
}

impl HintedSchedulerConfig {
    pub fn default_for_dev_network() -> Self {
        Self {
            check_interval: Duration::from_millis(100),
            block_cooldown: Duration::from_millis(100),
            ..Default::default()
        }
    }
}

impl Default for HintedSchedulerConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_millis(1000),
            block_cooldown: Duration::from_millis(5000),
            hinting_theshold_percent: 10,
        }
    }
}
