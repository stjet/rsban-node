use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::WalletId;
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = clap::ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct DecryptUnsafeArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl DecryptUnsafeArgs {
    pub(crate) fn decrypt_unsafe(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallet_id = WalletId::decode_hex(&self.wallet)
            .map_err(|e| anyhow!("Wallet id is invalid: {:?}", e))?;

        let wallets = Arc::new(
            Wallets::new_null(&path).map_err(|e| anyhow!("Failed to create wallets: {:?}", e))?,
        );

        let password = self.password.clone().unwrap_or_default();

        wallets.ensure_wallet_is_unlocked(wallet_id, &password);

        let seed = wallets
            .get_seed(wallet_id)
            .map_err(|e| anyhow!("Failed to get wallet seed: {:?}", e))?;

        println!("Seed: {:?}", seed);

        Ok(())
    }
}
