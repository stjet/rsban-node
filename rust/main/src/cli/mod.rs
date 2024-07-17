use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    account::AccountCommand,
    clear::{
        clear_send_ids::ClearSendIdsArgs, confirmation_height_clear::ConfirmationHeightClearArgs,
        final_vote_clear::FinalVoteClearArgs, online_weight_clear::OnlineWeightClearArgs,
        peer_clear::PeerClearArgs,
    },
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
            Some(Commands::OnlineWeightClear(args)) => args.online_weight_clear(),
            Some(Commands::PeerClear(args)) => args.peer_clear(),
            Some(Commands::ConfirmationHeightClear(args)) => args.confirmation_height_clear(),
            Some(Commands::ClearSendIds(args)) => args.clear_send_ids(),
            Some(Commands::FinalVoteClear(args)) => args.final_vote_clear()?,
            Some(Commands::Wallet(command)) => command.run()?,
            Some(Commands::Vacuum(args)) => args.vacuum()?,
            Some(Commands::RebuildDatabase(args)) => args.rebuild_database()?,
            Some(Commands::Snapshot(args)) => args.snapshot()?,
            Some(Commands::Account(command)) => command.run()?,
            Some(Commands::Node(command)) => command.run()?,
            Some(Commands::Key(command)) => command.run()?,
            None => Cli::command().print_long_help()?,
        }
        Ok(())
    }
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Either specify a single --root to clear or --all to clear all final votes (not recommended).
    FinalVoteClear(FinalVoteClearArgs),
    /// Account subcommands
    Account(AccountCommand),
    /// Key subcommands
    Key(KeyCommand),
    /// Node subcommands
    Node(NodeCommand),
    /// Wallet subcommands
    Wallet(WalletCommand),
    /// Clear online weight history records.
    OnlineWeightClear(OnlineWeightClearArgs),
    /// Remove all send IDs from the database (dangerous: not intended for production use).
    ClearSendIds(ClearSendIdsArgs),
    /// Clear online peers database dump.
    PeerClear(PeerClearArgs),
    /// Clear confirmation height. Requires an <account> option that can be 'all' to clear all accounts.
    ConfirmationHeightClear(ConfirmationHeightClearArgs),
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
