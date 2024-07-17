use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use rebuild::RebuildArgs;
use snapshot::SnapshotArgs;
use vacuum::VacuumArgs;

pub(crate) mod rebuild;
pub(crate) mod snapshot;
pub(crate) mod vacuum;

#[derive(Subcommand)]
pub(crate) enum DatabaseSubcommands {
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
}

#[derive(Parser)]
pub(crate) struct DatabaseCommand {
    #[command(subcommand)]
    pub subcommand: Option<DatabaseSubcommands>,
}

impl DatabaseCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(DatabaseSubcommands::Vacuum(args)) => args.vacuum()?,
            Some(DatabaseSubcommands::Rebuild(args)) => args.rebuild()?,
            Some(DatabaseSubcommands::Snapshot(args)) => args.snapshot()?,
            None => DatabaseCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
