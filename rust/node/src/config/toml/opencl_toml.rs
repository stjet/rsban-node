use serde::{Deserialize, Serialize};

use crate::config::OpenclConfig;

#[derive(Deserialize, Serialize)]
pub struct OpenclToml {
    pub enable: Option<bool>,
    pub platform: Option<u32>,
    pub device: Option<u32>,
    pub threads: Option<u32>,
}

impl OpenclToml {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for OpenclToml {
    fn default() -> Self {
        let config = OpenclConfig::default();
        (&config).into()
    }
}

impl From<&OpenclToml> for OpenclConfig {
    fn from(toml: &OpenclToml) -> Self {
        let mut config = OpenclConfig::default();
        if let Some(device) = toml.device {
            config.device = device;
        }
        if let Some(platform) = toml.platform {
            config.platform = platform;
        }
        if let Some(threads) = toml.threads {
            config.threads = threads;
        }
        config
    }
}

impl From<&OpenclConfig> for OpenclToml {
    fn from(config: &OpenclConfig) -> Self {
        let mut toml = OpenclToml::default();
        toml.platform = Some(config.platform);
        toml.device = Some(config.device);
        toml.threads = Some(config.threads);
        toml
    }
}
