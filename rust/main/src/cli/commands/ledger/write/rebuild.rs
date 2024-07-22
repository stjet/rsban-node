use crate::cli::get_path;
use anyhow::anyhow;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct RebuildArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl RebuildArgs {
    pub(crate) fn rebuild(&self) -> anyhow::Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open(&path)
            .build()
            .map_err(|e| anyhow!("Failed to open store: {:?}", e))?;

        store.rebuild_db(&mut store.tx_begin_write())
    }
}
