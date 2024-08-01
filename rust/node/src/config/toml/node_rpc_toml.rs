use crate::config::RpcChildProcessConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct RpcChildProcessToml {
    pub enable: Option<bool>,
    pub rpc_path: Option<PathBuf>,
}

impl RpcChildProcessToml {
    pub fn new() -> Self {
        let config = RpcChildProcessConfig::new();
        Self {
            enable: Some(config.enable),
            rpc_path: Some(config.rpc_path),
        }
    }
}

impl From<&RpcChildProcessConfig> for RpcChildProcessToml {
    fn from(config: &RpcChildProcessConfig) -> Self {
        Self {
            enable: Some(config.enable),
            rpc_path: Some(config.rpc_path.clone()),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct NodeRpcToml {
    pub enable: Option<bool>,
    pub enable_sign_hash: Option<bool>,
    pub child_process: Option<RpcChildProcessToml>,
}

impl NodeRpcToml {
    pub fn new() -> Self {
        Self {
            enable: Some(false),
            enable_sign_hash: Some(false),
            child_process: Some(RpcChildProcessToml::new()),
        }
    }
}
