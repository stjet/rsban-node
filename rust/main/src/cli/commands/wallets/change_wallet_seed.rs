use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::{RawKey, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct ChangeWalletSeedArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    seed: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl ChangeWalletSeedArgs {
    pub(crate) fn change_wallet_seed(&self) -> Result<()> {
        let wallet_id = WalletId::decode_hex(&self.wallet)?;

        let seed = RawKey::decode_hex(&self.seed)?;

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path)?);

        let password = self.password.clone().unwrap_or_default();

        wallets.ensure_wallet_is_unlocked(wallet_id, &password);

        wallets
            .change_seed(wallet_id, &seed, 0)
            .map_err(|e| anyhow!("Failed to change wallet seed: {:?}", e))?;

        Ok(())
    }
}
