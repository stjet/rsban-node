use crate::cli::get_path;
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
        let data_path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open_existing(&data_path).unwrap();

        // prune

        Ok(())
    }
}
