use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct PeersArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl PeersArgs {
    pub(crate) fn peers(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let store =
            LmdbStore::open_existing(&path).map_err(|e| anyhow!("Error opening store: {:?}", e))?;

        let mut txn = store.tx_begin_read();
        for peer in store.peer.iter(&mut txn) {
            println!("{:?}", peer);
        }

        Ok(())
    }
}
