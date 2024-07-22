use crate::cli::get_path;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
pub(crate) struct ValidateBlocksArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl ValidateBlocksArgs {
    pub(crate) fn validate_blocks(&self) -> anyhow::Result<()> {
        let data_path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store = LmdbStore::open_existing(&data_path).unwrap();

        store.rebuild_db(&mut store.tx_begin_write())
    }
}
