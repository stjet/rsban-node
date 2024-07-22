use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::Amount;
use rsnano_ledger::{Ledger, LedgerCache, RepWeightCache};
use rsnano_node::{config::NetworkConstants, NetworkParams};
use rsnano_store_lmdb::LmdbStore;
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct RepresentativesArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl RepresentativesArgs {
    pub(crate) fn dump_representatives(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        let network_params = NetworkParams::new(NetworkConstants::active_network());

        let ledger_cache = Arc::new(LedgerCache::new());

        let ledger = Ledger::new(
            Arc::new(
                LmdbStore::open_existing(&path)
                    .map_err(|e| anyhow!("Failed to open store: {:?}", e))?,
            ),
            network_params.ledger,
            Amount::zero(),
            Arc::new(RepWeightCache::new()),
            ledger_cache,
        )?;

        let rep_amounts = ledger.rep_weights.read().to_owned();
        let mut total = Amount::zero();

        for (account, amount) in rep_amounts {
            total += amount;
            println!(
                "{} {} {}",
                account,
                account.encode_account(),
                total.number()
            );
        }

        Ok(())
    }
}
