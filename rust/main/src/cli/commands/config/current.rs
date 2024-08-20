use crate::cli::get_path;
use anyhow::Result;
use clap::{ArgGroup, Parser};
use rsnano_node::config::{get_node_toml_config_path, get_rpc_toml_config_path, DaemonToml};
use rsnano_rpc::RpcToml;
use std::fs::read_to_string;
use toml::from_str;

#[derive(Parser)]
#[command(group = ArgGroup::new("input1")
    .args(&["node", "rpc"])
    .required(true))]
#[command(group = ArgGroup::new("input2")
    .args(&["data_path", "network"]))]
pub(crate) struct CurrentArgs {
    /// Prints the current node config
    #[arg(long, group = "input1")]
    node: bool,
    /// Prints the current rpc config
    #[arg(long, group = "input1")]
    rpc: bool,
    /// Uses the supplied path as the data directory
    #[arg(long, group = "input2")]
    data_path: Option<String>,
    /// Uses the supplied network (live, test, beta or dev)
    #[arg(long, group = "input2")]
    network: Option<String>,
}

impl CurrentArgs {
    pub(crate) fn current(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network);

        if self.node {
            let node_toml_config_path = get_node_toml_config_path(&path);

            if node_toml_config_path.exists() {
                let daemon_toml_str = read_to_string(&node_toml_config_path)?;

                let current_daemon_toml: DaemonToml = from_str(&daemon_toml_str)?;

                let default_daemon_toml = DaemonToml::default();

                let merged_toml_str = current_daemon_toml.merge_defaults(&default_daemon_toml)?;

                println!("{}", merged_toml_str);
            }
        } else {
            let rpc_toml_config_path = get_rpc_toml_config_path(&path);

            if rpc_toml_config_path.exists() {
                let toml_str = read_to_string(&rpc_toml_config_path)?;

                let current_rpc_toml: RpcToml = from_str(&toml_str)?;

                let default_rpc_toml = RpcToml::default();

                let merged_toml_str = current_rpc_toml.merge_defaults(&default_rpc_toml)?;

                println!("{}", merged_toml_str);
            }
        }

        Ok(())
    }
}
