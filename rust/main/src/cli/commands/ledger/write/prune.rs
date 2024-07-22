use crate::cli::get_path;
use anyhow::anyhow;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct PruneArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl PruneArgs {
    pub(crate) fn prune(&self) -> anyhow::Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open(&path)
            .build()
            .map_err(|e| anyhow!("Failed to open store: {:?}", e))?;

        // prune

        Ok(())
    }
}
