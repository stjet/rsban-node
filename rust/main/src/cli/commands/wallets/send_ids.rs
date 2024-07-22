use std::sync::Arc;

use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_node::wallets::Wallets;
use rsnano_store_lmdb::LmdbEnv;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct SendIdsArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl SendIdsArgs {
    pub(crate) fn send_ids(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let env = Arc::new(LmdbEnv::new(&path)?);

        let wallets = Wallets::new_with_env(env)?;

        wallets.clear_send_ids();

        println!("{}", "Send IDs deleted");

        Ok(())
    }
}
