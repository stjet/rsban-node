use crate::{RpcConfig, RpcLoggingConfig, RpcProcessConfig};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct RpcLoggingToml {
    pub log_rpc: Option<bool>,
}

impl Default for RpcLoggingToml {
    fn default() -> Self {
        let config = RpcLoggingConfig::new();
        (&config).into()
    }
}

impl From<&RpcLoggingConfig> for RpcLoggingToml {
    fn from(config: &RpcLoggingConfig) -> Self {
        Self {
            log_rpc: Some(config.log_rpc),
        }
    }
}

impl From<&RpcLoggingToml> for RpcLoggingConfig {
    fn from(toml: &RpcLoggingToml) -> Self {
        let mut config = RpcLoggingConfig::new();
        if let Some(log_rpc) = toml.log_rpc {
            config.log_rpc = log_rpc;
        }
        config
    }
}

#[derive(Deserialize, Serialize)]
pub struct RpcProcessToml {
    pub io_threads: Option<u32>,
    pub ipc_address: Option<String>,
    pub ipc_port: Option<u16>,
    pub num_ipc_connections: Option<u32>,
}

impl Default for RpcProcessToml {
    fn default() -> Self {
        let config = RpcProcessConfig::default();
        (&config).into()
    }
}

impl From<&RpcProcessConfig> for RpcProcessToml {
    fn from(config: &RpcProcessConfig) -> Self {
        Self {
            io_threads: Some(config.io_threads),
            ipc_address: Some(config.ipc_address.clone()),
            ipc_port: Some(config.ipc_port),
            num_ipc_connections: Some(config.num_ipc_connections),
        }
    }
}

impl From<&RpcProcessToml> for RpcProcessConfig {
    fn from(toml: &RpcProcessToml) -> Self {
        let mut config = RpcProcessConfig::default();
        if let Some(io_threads) = toml.io_threads {
            config.io_threads = io_threads;
        }
        if let Some(ipc_address) = &toml.ipc_address {
            config.ipc_address = ipc_address.clone();
        }
        if let Some(ipc_port) = toml.ipc_port {
            config.ipc_port = ipc_port;
        }
        if let Some(num_ipc_connections) = toml.num_ipc_connections {
            config.num_ipc_connections = num_ipc_connections;
        }
        config
    }
}

#[derive(Deserialize, Serialize)]
pub struct RpcToml {
    pub address: Option<String>,
    pub port: Option<u16>,
    pub enable_control: Option<bool>,
    pub max_json_depth: Option<u8>,
    pub max_request_size: Option<u64>,
    pub logging: Option<RpcLoggingToml>,
    pub process: Option<RpcProcessToml>,
}

impl From<&RpcToml> for RpcConfig {
    fn from(toml: &RpcToml) -> Self {
        let mut config = RpcConfig::default();
        if let Some(address) = &toml.address {
            config.address = address.clone();
        }
        if let Some(port) = toml.port {
            config.port = port;
        }
        if let Some(enable_control) = toml.enable_control {
            config.enable_control = enable_control;
        }
        if let Some(max_json_depth) = toml.max_json_depth {
            config.max_json_depth = max_json_depth;
        }
        if let Some(max_request_size) = toml.max_request_size {
            config.max_request_size = max_request_size;
        }
        if let Some(logging) = &toml.logging {
            config.rpc_logging = logging.into();
        }
        if let Some(process) = &toml.process {
            config.rpc_process = process.into();
        }
        config
    }
}

impl From<&RpcConfig> for RpcToml {
    fn from(config: &RpcConfig) -> Self {
        Self {
            address: Some(config.address.clone()),
            port: Some(config.port),
            enable_control: Some(config.enable_control),
            max_json_depth: Some(config.max_json_depth),
            max_request_size: Some(config.max_request_size),
            logging: Some((&config.rpc_logging).into()),
            process: Some((&config.rpc_process).into()),
        }
    }
}

impl Default for RpcToml {
    fn default() -> Self {
        let config = RpcConfig::default();
        (&config).into()
    }
}
