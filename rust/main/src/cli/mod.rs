use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    account::AccountCommand,
    clear::ClearCommand,
    database::{rebuild::RebuildDatabaseArgs, snapshot::SnapshotArgs, vacuum::VacuumArgs},
    key::KeyCommand,
    node::NodeCommand,
    wallet::WalletCommand,
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
            Some(Commands::Vacuum(args)) => args.vacuum()?,
            Some(Commands::RebuildDatabase(args)) => args.rebuild_database()?,
            Some(Commands::Snapshot(args)) => args.snapshot()?,
            Some(Commands::Account(command)) => command.run()?,
            Some(Commands::Node(command)) => command.run()?,
            Some(Commands::Key(command)) => command.run()?,
            Some(Commands::Clear(command)) => command.run()?,
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
    /// Compact database. If data_path is missing, the database in the data directory is compacted.
    /// Optional: --unchecked_clear, --clear_send_ids, --online_weight_clear, --peer_clear, --confirmation_height_clear, --rebuild_database.
    /// Requires approximately data.ldb size * 2 free space on disk.
    Vacuum(VacuumArgs),
    /// Rebuild LMDB database with --vacuum for best compaction.
    /// Requires approximately data.ldb size * 2 free space on disk.
    RebuildDatabase(RebuildDatabaseArgs),
    /// Compact database and create snapshot, functions similar to vacuum but does not replace the existing database.
    Snapshot(SnapshotArgs),
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
