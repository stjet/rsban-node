use crate::cli::get_path;
use anyhow::anyhow;
use anyhow::Result;
use clap::{ArgGroup, Parser};
use rsnano_core::Amount;
use rsnano_ledger::{Ledger, LedgerCache, RepWeightCache};
use rsnano_node::{config::NetworkConstants, NetworkParams, OnlineWeightSampler};
use rsnano_store_lmdb::LmdbStore;
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

        let network_params = NetworkParams::new(NetworkConstants::active_network());

        let ledger_cache = Arc::new(LedgerCache::new());

        let ledger = Arc::new(Ledger::new(
            Arc::new(
                LmdbStore::open_existing(&path)
                    .map_err(|e| anyhow!("Failed to open store: {:?}", e))?,
            ),
            network_params.ledger,
            Amount::zero(),
            Arc::new(RepWeightCache::new()),
            ledger_cache,
        )?);

        let sampler = OnlineWeightSampler::new(ledger.clone());

        let current = sampler.calculate_trend().number();

        println!("Trended Weight {}", current);

        let mut txn = ledger.store.tx_begin_read();

        let mut iter = ledger.store.online_weight.begin(&mut txn);

        loop {
            match iter.current() {
                Some((timestamp, amount)) => {
                    println!("Timestamp {} Weight {}", timestamp, amount.number());
                }
                None => break,
            }
            iter.next();
        }

        Ok(())
    }
}
