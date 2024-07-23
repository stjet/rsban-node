use crate::cli::get_path;
use anyhow::anyhow;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct ValidateBlocksArgs {
    /// Uses the supplied path as the data directory
    #[arg(long, group = "input")]
    data_path: Option<String>,
    /// Uses the supplied network (live, test, beta or dev)
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl ValidateBlocksArgs {
    pub(crate) fn validate_blocks(&self) -> anyhow::Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open(&path)
            .build()
            .map_err(|e| anyhow!("Failed to open store: {:?}", e))?;

        Ok(())
    }
}
