use super::rpc_config::{RpcConfig, RpcLoggingConfig, RpcProcessConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct RpcToml {
    pub address: Option<String>,
    pub port: Option<u16>,
    pub enable_control: Option<bool>,
    pub max_json_depth: Option<u8>,
    pub max_request_size: Option<u64>,
    pub rpc_logging: Option<RpcLoggingToml>,
    pub rpc_process: Option<RpcProcessToml>,
}

impl RpcToml {
    pub fn merge_defaults(&self, default_config: &RpcToml) -> Result<String> {
        let defaults_str = toml::to_string(default_config)?;
        let current_str = toml::to_string(self)?;

        let mut result = String::new();
        let mut stream_defaults = defaults_str.lines().peekable();
        let mut stream_current = current_str.lines().peekable();

        while stream_current.peek().is_some() || stream_defaults.peek().is_some() {
            match (stream_defaults.peek(), stream_current.peek()) {
                (Some(&line_defaults), Some(&line_current)) => {
                    if line_defaults == line_current {
                        result.push_str(line_defaults);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    } else if line_current.starts_with('#') {
                        result.push_str("# ");
                        result.push_str(line_defaults);
                        result.push('\n');

                        result.push_str(line_current);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    } else {
                        result.push_str("# ");
                        result.push_str(line_defaults);
                        result.push('\n');
                        result.push_str(line_current);
                        result.push('\n');
                        stream_defaults.next();
                        stream_current.next();
                    }
                }
                (Some(&line_defaults), None) => {
                    result.push_str("# ");
                    result.push_str(line_defaults);
                    result.push('\n');
                    stream_defaults.next();
                }
                (None, Some(&line_current)) => {
                    result.push_str(line_current);
                    result.push('\n');
                    stream_current.next();
                }
                _ => {}
            }
        }

        Ok(result)
    }
}

impl Default for RpcToml {
    fn default() -> Self {
        let config = RpcConfig::default();
        Self {
            address: Some(config.address),
            port: Some(config.port),
            enable_control: Some(config.enable_control),
            max_json_depth: Some(config.max_json_depth),
            max_request_size: Some(config.max_request_size),
            rpc_logging: Some((&config.rpc_logging).into()),
            rpc_process: Some((&config.rpc_process).into()),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RpcLoggingToml {
    pub log_rpc: Option<bool>,
}

#[derive(Deserialize, Serialize)]
pub struct RpcProcessToml {
    pub io_threads: Option<u32>,
    pub ipc_address: Option<String>,
    pub ipc_port: Option<u16>,
    pub num_ipc_connections: Option<u32>,
}

impl From<&RpcConfig> for RpcToml {
    fn from(config: &RpcConfig) -> Self {
        RpcToml {
            address: Some(config.address.clone()),
            port: Some(config.port),
            enable_control: Some(config.enable_control),
            max_json_depth: Some(config.max_json_depth),
            max_request_size: Some(config.max_request_size),
            rpc_logging: Some((&config.rpc_logging).into()),
            rpc_process: Some((&config.rpc_process).into()),
        }
    }
}

impl From<&RpcLoggingConfig> for RpcLoggingToml {
    fn from(config: &RpcLoggingConfig) -> Self {
        RpcLoggingToml {
            log_rpc: Some(config.log_rpc),
        }
    }
}

impl From<&RpcProcessConfig> for RpcProcessToml {
    fn from(config: &RpcProcessConfig) -> Self {
        RpcProcessToml {
            io_threads: Some(config.io_threads),
            ipc_address: Some(config.ipc_address.clone()),
            ipc_port: Some(config.ipc_port),
            num_ipc_connections: Some(config.num_ipc_connections),
        }
    }
}
