use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct CreateArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl CreateArgs {
    pub(crate) fn create(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(
            Wallets::new_null(&path).map_err(|e| anyhow!("Failed to create wallets: {:?}", e))?,
        );

        let wallet = WalletId::decode_hex(&self.wallet)
            .map_err(|e| anyhow!("Wallet id is invalid: {:?}", e))?;

        if self.password.is_none() {
            wallets.ensure_wallet_is_unlocked(wallet, "");
        }

        let public_key = wallets
            .deterministic_insert2(&wallet, false)
            .map_err(|e| anyhow!("Failed to insert wallet: {:?}", e))?;

        println!("Account: {:?}", Account::encode_account(&public_key));

        Ok(())
    }
}
