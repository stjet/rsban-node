use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clear::ClearCommand;
use info::InfoCommand;
use rebuild::RebuildArgs;
use snapshot::SnapshotArgs;
use vacuum::VacuumArgs;

pub(crate) mod clear;
pub(crate) mod info;
pub(crate) mod rebuild;
pub(crate) mod snapshot;
pub(crate) mod vacuum;

#[derive(Subcommand)]
pub(crate) enum LedgerSubcommands {
    /// Compact database. If data_path is missing, the database in the data directory is compacted.
    ///
    /// Optional: --unchecked_clear, --clear_send_ids, --online_weight_clear, --peer_clear, --confirmation_height_clear, --rebuild_database.
    /// Requires approximately data.ldb size * 2 free space on disk.
    Vacuum(VacuumArgs),
    /// Rebuild LMDB database with --vacuum for best compaction.
    ///
    /// Requires approximately data.ldb size * 2 free space on disk.
    Rebuild(RebuildArgs),
    /// Compact database and create snapshot, functions similar to vacuum but does not replace the existing database.
    Snapshot(SnapshotArgs),
    /// Commands that clear some part of the ledger
    Clear(ClearCommand),
    /// Commands that get some info from the ledger
    Info(InfoCommand),
}

#[derive(Parser)]
pub(crate) struct LedgerCommand {
    #[command(subcommand)]
    pub subcommand: Option<LedgerSubcommands>,
}

impl LedgerCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(LedgerSubcommands::Vacuum(args)) => args.vacuum()?,
            Some(LedgerSubcommands::Rebuild(args)) => args.rebuild()?,
            Some(LedgerSubcommands::Snapshot(args)) => args.snapshot()?,
            Some(LedgerSubcommands::Clear(command)) => command.run()?,
            Some(LedgerSubcommands::Info(command)) => command.run()?,
            None => LedgerCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
