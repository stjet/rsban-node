use crate::monitor::MonitorConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize, Serialize)]
pub struct MonitorToml {
    pub enabled: Option<bool>,
    pub interval: Option<u64>,
}

impl Default for MonitorToml {
    fn default() -> Self {
        let config = MonitorConfig::default();
        Self {
            enabled: Some(config.enabled),
            interval: Some(config.interval.as_secs()),
        }
    }
}

impl From<&MonitorToml> for MonitorConfig {
    fn from(toml: &MonitorToml) -> Self {
        let mut config = MonitorConfig::default();

        if let Some(enabled) = toml.enabled {
            config.enabled = enabled;
        }
        if let Some(interval) = &toml.interval {
            config.interval = Duration::from_secs(*interval);
        }
        config
    }
}

impl From<&MonitorConfig> for MonitorToml {
    fn from(config: &MonitorConfig) -> Self {
        Self {
            enabled: Some(config.enabled),
            interval: Some(config.interval.as_secs()),
        }
    }
}
