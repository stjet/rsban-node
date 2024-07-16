use clap::Parser;

#[derive(Parser)]
pub(crate) struct RebuildDatabaseArgs;

impl RebuildDatabaseArgs {
    pub(crate) fn rebuild_database(&self) {}
}
