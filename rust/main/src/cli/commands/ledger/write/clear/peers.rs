use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::{LmdbEnv, LmdbPeerStore};
use std::sync::Arc;

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

        let env = Arc::new(LmdbEnv::new(&path)?);

        let peers_store = LmdbPeerStore::new(env.clone())
            .map_err(|e| anyhow!("Failed to open peers database: {:?}", e))?;

        let mut txn = env.tx_begin_write();

        peers_store.clear(&mut txn);

        println!("{}", "Peers were cleared from the database");

        Ok(())
    }
}
