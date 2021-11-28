use anyhow::Result;
use std::{
    net::Ipv6Addr,
    path::{Path, PathBuf},
};

use crate::utils::TomlWriter;

use super::NetworkConstants;

pub fn get_default_rpc_filepath() -> Result<PathBuf> {
    Ok(get_default_rpc_filepath_from(
        std::env::current_exe()?.as_path(),
    ))
}

fn get_default_rpc_filepath_from(node_exe_path: &Path) -> PathBuf {
    let mut result = node_exe_path.to_path_buf();
    result.pop();
    result.push("nano_rpc");
    if let Some(ext) = node_exe_path.extension() {
        result.set_extension(ext);
    }
    result
}

pub struct RpcConfig {
    pub address: String,
    pub port: u16,
    pub enable_control: bool,
}

impl RpcConfig {
    pub fn new(network_constants: &NetworkConstants) -> Self {
        Self::new2(network_constants, network_constants.default_rpc_port, false)
    }

    pub fn new2(network_constants: &NetworkConstants, port: u16, enable_control: bool) -> Self {
        Self {
            address: Ipv6Addr::LOCALHOST.to_string(),
            port,
            enable_control,
        }
    }

    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> Result<()> {
        toml.put_str("address", &self.address, "Bind address for the RPC server.\ntype:string,ip")?;
        toml.put_u16("port", self.port, "Listening port for the RPC server.\ntype:uint16")?;
        toml.put_bool("enable_control", self.enable_control, "Enable or disable control-level requests.\nWARNING: Enabling this gives anyone with RPC access the ability to stop the node and access wallet funds.\ntype:bool")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rpc_filepath() -> Result<()> {
        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/path/to/nano_node")),
            Path::new("/path/to/nano_rpc")
        );

        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/nano_node")),
            Path::new("/nano_rpc")
        );

        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/bin/nano_node.exe")),
            Path::new("/bin/nano_rpc.exe")
        );

        Ok(())
    }
}
