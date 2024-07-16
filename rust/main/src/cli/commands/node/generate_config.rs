use clap::Parser;
use rsnano_core::work::WorkThresholds;
use rsnano_node::{
    config::{DaemonConfig, NetworkConstants, RpcConfig},
    utils::TomlConfig,
    NetworkParams,
};
use std::path::PathBuf;

#[derive(Parser)]
pub(crate) struct GenerateConfigOptions {
    #[arg(short, long)]
    config_type: String,
    #[arg(short, long)]
    use_defaults: bool,
    #[arg(short, long)]
    data_path: PathBuf,
}

impl GenerateConfigOptions {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let mut valid_type = false;
        let mut toml = TomlConfig::new();
        let network = NetworkConstants::active_network();

        if self.config_type == "node" {
            valid_type = true;
            let network_params = NetworkParams::new(network);
            let mut config = DaemonConfig::new(&network_params, 0)?;
            config.node.peering_port = Some(network_params.network.default_node_port);
            config.serialize_toml(&mut toml)?
        } else if self.config_type == "rpc" {
            let network_constants = NetworkConstants::new(WorkThresholds::new(0, 0, 0), network);
            valid_type = true;
            let config = RpcConfig::new(&network_constants, 0);
            config.serialize_toml(&mut toml)?
        } else {
            eprintln!(
                "Invalid configuration type {}. Must be node or rpc.",
                self.config_type
            );
        }

        if valid_type {
            println!("# This is an example configuration file for Nano. Visit https://docs.nano.org/running-a-node/configuration/ for more information.\n#");
            println!(
                "# Fields may need to be defined in the context of a [category] above them.\n"
            );
            println!("# The desired configuration changes should be placed in config-{}.toml in the node data path.\n", self.config_type);
            println!("# To change a value from its default, uncomment (erasing #) the corresponding field.\n");
            println!("# It is not recommended to uncomment every field, as the default value for important fields may change in the future. Only change what you need.\n");
            println!("# Additional information for notable configuration options is available in https://docs.nano.org/running-a-node/configuration/#notable-configuration-options\n");

            println!("{}", toml.to_string());
        }
        Ok(())
    }
}
