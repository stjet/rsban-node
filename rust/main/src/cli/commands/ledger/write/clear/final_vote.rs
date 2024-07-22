use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::Root;
use rsnano_store_lmdb::{LmdbEnv, LmdbFinalVoteStore};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input1")
    .args(&["root", "all"])
    .required(true))]
#[command(group = ArgGroup::new("input2")
    .args(&["data_path", "network"]))]
pub(crate) struct FinalVoteArgs {
    #[arg(long, group = "input1")]
    root: Option<String>,
    #[arg(long, group = "input1")]
    all: bool,
    #[arg(long, group = "input2")]
    data_path: Option<String>,
    #[arg(long, group = "input2")]
    network: Option<String>,
}

impl FinalVoteArgs {
    pub(crate) fn final_vote(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let env = Arc::new(LmdbEnv::new(&path)?);

        let final_vote_store = LmdbFinalVoteStore::new(env.clone())
            .map_err(|e| anyhow!("Failed to open final vote database: {:?}", e))?;

        let mut txn = env.tx_begin_write();

        if let Some(root) = &self.root {
            match Root::decode_hex(root) {
                Ok(root_decoded) => {
                    final_vote_store.del(&mut txn, &root_decoded);
                    println!("Successfully cleared final vote");
                }
                Err(_) => {
                    println!("Invalid root");
                }
            }
        } else if self.all {
            final_vote_store.clear(&mut txn);
            println!("All final votes were cleared from the database");
        }

        Ok(())
    }
}
