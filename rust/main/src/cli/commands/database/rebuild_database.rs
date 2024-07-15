use clap::Parser;

#[derive(Parser)]
pub(crate) struct RebuildDatabaseOptions;

impl RebuildDatabaseOptions {
    pub(crate) fn run(&self) {}
}
