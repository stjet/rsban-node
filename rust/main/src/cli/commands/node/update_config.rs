use clap::{ArgGroup, Parser};
use rsnano_core::work::WorkThresholds;
use rsnano_node::{
    config::{DaemonConfig, NetworkConstants, RpcConfig},
    utils::TomlConfig,
    NetworkParams,
};

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["node", "rpc"])
    .required(true))]
pub(crate) struct UpdateConfigArgs {
    /// Updates the node config
    #[arg(long, group = "input")]
    node: bool,
    /// Updates the rpc config
    #[arg(long, group = "input")]
    rpc: bool,
}

impl UpdateConfigArgs {
    pub(crate) fn update_config(&self) -> anyhow::Result<()> {
        let mut toml = TomlConfig::new();
        let network = NetworkConstants::active_network();
        let mut config_type = "node";

        if self.node {
            let network_params = NetworkParams::new(network);
            let mut config = DaemonConfig::new(&network_params, 0)?;
            config.node.peering_port = Some(network_params.network.default_node_port);
            config.serialize_toml(&mut toml)?
        } else {
            config_type = "rpc";
            let network_constants = NetworkConstants::new(WorkThresholds::new(0, 0, 0), network);
            let config = RpcConfig::new(&network_constants, 0);
            config.serialize_toml(&mut toml)?
        }

        Ok(())
    }
}
