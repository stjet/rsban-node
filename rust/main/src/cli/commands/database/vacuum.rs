use std::fs;

use anyhow::Context;
use clap::Parser;
use rsnano_store_lmdb::LmdbStore;

use crate::cli::get_path;

#[derive(Parser)]
pub(crate) struct VacuumArgs {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl VacuumArgs {
    pub(crate) fn vacuum(&self) -> anyhow::Result<()> {
        let data_path = get_path(&self.data_path, &self.network);
        let source_path = data_path.join("data.ldb");
        let backup_path = data_path.join("backup.vacuum.ldb");
        let vacuum_path = data_path.join("vacuumed.ldb");

        println!("Vacuuming database copy in {:?}", data_path);
        println!("This may take a while...");

        let store = LmdbStore::open_existing(&source_path).unwrap();

        match store.copy_db(&vacuum_path) {
            Ok(_) => {
                println!("Finalizing");
                fs::remove_file(&backup_path).context("Failed to remove backup file")?;
                fs::rename(&source_path, &backup_path)
                    .context("Failed to rename source to backup")?;
                fs::rename(&vacuum_path, &source_path)
                    .context("Failed to rename vacuum to source")?;
                println!("Vacuum completed");
            }
            Err(e) => {
                eprintln!("Vacuum failed: {}", e);
            }
        }

        Ok(())
    }
}
