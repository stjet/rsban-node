use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    ledger::LedgerCommand, node::NodeCommand, utils::UtilsCommand, wallets::WalletsCommand,
};
use rsnano_core::Networks;
use rsnano_node::{config::NetworkConstants, working_path};
use std::{path::PathBuf, str::FromStr};

mod commands;

#[derive(Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.command {
            Some(Commands::Wallets(command)) => command.run()?,
            Some(Commands::Utils(command)) => command.run()?,
            Some(Commands::Node(command)) => command.run()?,
            Some(Commands::Ledger(command)) => command.run()?,
            //Some(Commands::Debug(command)) => command.run()?,
            None => Cli::command().print_long_help()?,
        }
        Ok(())
    }
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Commands related to the protocol, such as keys and accounts
    Utils(UtilsCommand),
    /// Commands to read from and write to the ledger
    Ledger(LedgerCommand),
    /// Commands related to running the node
    Node(NodeCommand),
    /// Commands to manage wallets
    Wallets(WalletsCommand),
    // Debug command
    //Debug(DebugCommand),
}

pub(crate) fn get_path(path_str: &Option<String>, network_str: &Option<String>) -> PathBuf {
    if let Some(path) = path_str {
        return PathBuf::from_str(path).unwrap();
    }
    if let Some(network) = network_str {
        let network = Networks::from_str(&network).unwrap();
        NetworkConstants::set_active_network(network);
    }
    working_path().unwrap()
}
