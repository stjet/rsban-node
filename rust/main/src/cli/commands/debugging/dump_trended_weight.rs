use crate::cli::get_path;
use anyhow::Result;
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::{LmdbEnv, LmdbOnlineWeightStore};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct DumpTrendedWeightArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl DumpTrendedWeightArgs {
    pub(crate) fn dump_trended_weight(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let env = LmdbEnv::new(&path).unwrap();

        let mut txn = env.tx_begin_read();

        let store = LmdbOnlineWeightStore::new(Arc::new(env))?;

        let mut iter = store.begin(&mut txn);
        let end = store.end();

        while iter != end {
            match iter.current() {
                Some((timestamp, amount)) => {
                    println!("Timestamp {} Weight {:?}", timestamp, amount);
                }
                None => break,
            }
            iter.next();
        }

        Ok(())
    }
}
