use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clear::ClearCommand;
use prune::PruneArgs;
use rebuild::RebuildArgs;
use snapshot::SnapshotArgs;
use vacuum::VacuumArgs;

pub(crate) mod clear;
pub(crate) mod prune;
pub(crate) mod rebuild;
pub(crate) mod snapshot;
pub(crate) mod vacuum;

#[derive(Subcommand)]
pub(crate) enum WriteSubcommands {
    /// Compacts the database. If data_path is missing, the database in the data directory is compacted.
    ///
    /// Optional: --unchecked_clear, --clear_send_ids, --online_weight_clear, --peer_clear, --confirmation_height_clear, --rebuild_database.
    /// Requires approximately data.ldb size * 2 free space on disk.
    Vacuum(VacuumArgs),
    /// Rebuilds the database with vacuum for best compaction.
    ///
    /// Requires approximately data.ldb size * 2 free space on disk.
    Rebuild(RebuildArgs),
    /// Compacts the database and creates a snapshot, functions similar to vacuum but does not replace the existing database.
    Snapshot(SnapshotArgs),
    /// Commands that clear some component of the ledger
    Clear(ClearCommand),
    /// Prunes accounts up to last confirmed blocks (EXPERIMENTAL)
    Prune(PruneArgs),
}

#[derive(Parser)]
pub(crate) struct WriteCommand {
    #[command(subcommand)]
    pub subcommand: Option<WriteSubcommands>,
}

impl WriteCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(WriteSubcommands::Vacuum(args)) => args.vacuum()?,
            Some(WriteSubcommands::Rebuild(args)) => args.rebuild()?,
            Some(WriteSubcommands::Snapshot(args)) => args.snapshot()?,
            Some(WriteSubcommands::Clear(command)) => command.run()?,
            Some(WriteSubcommands::Prune(args)) => args.prune()?,
            None => WriteCommand::command().print_long_help()?,
        }
        Ok(())
    }
}
