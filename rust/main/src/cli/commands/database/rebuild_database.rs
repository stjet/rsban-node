use crate::cli::get_path;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct RebuildDatabaseArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl RebuildDatabaseArgs {
    pub(crate) fn rebuild_database(&self) -> anyhow::Result<()> {
        let data_path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open_existing(&data_path).unwrap();

        store.rebuild_db(&mut store.tx_begin_write())
    }
}
