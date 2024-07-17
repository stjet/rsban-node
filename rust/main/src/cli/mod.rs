use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    account::AccountCommand, clear::ClearCommand, database::DatabaseCommand,
    debugging::DebugCommand, key::KeyCommand, node::NodeCommand, wallet::WalletCommand,
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
            Some(Commands::Wallet(command)) => command.run()?,
            Some(Commands::Database(command)) => command.run()?,
            Some(Commands::Account(command)) => command.run()?,
            Some(Commands::Node(command)) => command.run()?,
            Some(Commands::Key(command)) => command.run()?,
            Some(Commands::Clear(command)) => command.run()?,
            Some(Commands::Debug(command)) => command.run()?,
            None => Cli::command().print_long_help()?,
        }
        Ok(())
    }
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Clear command
    Clear(ClearCommand),
    /// Account command
    Account(AccountCommand),
    /// Key command
    Key(KeyCommand),
    /// Node command
    Node(NodeCommand),
    /// Wallet command
    Wallet(WalletCommand),
    /// Database command
    Database(DatabaseCommand),
    /// Debug command
    Debug(DebugCommand),
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
