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
pub(crate) struct DumpFrontierUncheckedDependentsArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl DumpFrontierUncheckedDependentsArgs {
    pub(crate) fn dump_frontier_unchecked_dependents(&self) -> Result<()> {
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

        println!("Outputting any frontier hashes which have associated key hashes in the unchecked table (may take some time)...");

        // Cache the account heads to make searching quicker against unchecked keys.
        /*let transaction = ledger.tx_begin_read();
        let mut frontier_hashes: HashSet<BlockHash> = HashSet::new();

        for (account, info) in ledger.account_begin(&transaction) {
            frontier_hashes.insert(info.head);
        }

        // Check all unchecked keys for matching frontier hashes. Indicates an issue with process_batch algorithm.
        node.unchecked.for_each(|key, _info| {
            if frontier_hashes.contains(&key.key) {
                println!("{}", key.key.to_string());
            }
            });
            }*/

        Ok(())
    }
}
