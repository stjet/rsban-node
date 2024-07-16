use clap::Parser;

#[derive(Parser)]
pub(crate) struct SnapshotArgs;

impl SnapshotArgs {
    pub(crate) fn snapshot(&self) {}
}
