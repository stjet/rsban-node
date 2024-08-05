use rsnano_store_lmdb::{LmdbConfig, SyncStrategy};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct LmdbToml {
    pub map_size: Option<usize>,
    pub max_databases: Option<u32>,
    pub sync: Option<SyncStrategy>,
}

impl Default for LmdbToml {
    fn default() -> Self {
        let config = LmdbConfig::default();
        (&config).into()
    }
}

impl From<&LmdbToml> for LmdbConfig {
    fn from(toml: &LmdbToml) -> Self {
        let mut config = LmdbConfig::default();

        if let Some(sync) = toml.sync {
            config.sync = sync;
        }
        if let Some(max_databases) = toml.max_databases {
            config.max_databases = max_databases;
        }
        if let Some(map_size) = toml.map_size {
            config.map_size = map_size;
        }
        config
    }
}

impl From<&LmdbConfig> for LmdbToml {
    fn from(config: &LmdbConfig) -> Self {
        Self {
            sync: Some(config.sync),
            max_databases: Some(config.max_databases),
            map_size: Some(config.map_size),
        }
    }
}
