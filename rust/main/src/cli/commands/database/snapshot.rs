use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

use crate::cli::get_path;

#[derive(Parser)]
pub(crate) struct SnapshotArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl SnapshotArgs {
    pub(crate) fn snapshot(&self) -> anyhow::Result<()> {
        let source_path = get_path(&self.data_path, &self.network).join("data.ldb");
        let snapshot_path = get_path(&self.data_path, &self.network).join("snapshot.ldb");

        let store = LmdbStore::open_existing(&source_path).unwrap();
        store.copy_db(&snapshot_path)?;

        println!(
            "Database snapshot of {:?} to {:?} in progress",
            source_path, snapshot_path
        );
        println!("This may take a while...");

        Ok(())
    }
}
