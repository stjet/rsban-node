use crate::cli::get_path;
use anyhow::Result;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct PruneArgs {
    /// Uses the supplied path as the data directory
    #[arg(long, group = "input")]
    data_path: Option<String>,
    /// Uses the supplied network (live, test, beta or dev)
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl PruneArgs {
    pub(crate) fn prune(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open(&path).build()?;

        // prune

        Ok(())
    }
}
