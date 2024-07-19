use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct RepresentativeSetArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    account: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl RepresentativeSetArgs {
    pub(crate) fn representative_set(&self) -> Result<()> {
        let wallet_id = WalletId::decode_hex(&self.wallet)
            .map_err(|e| anyhow!("Wallet id is invalid: {:?}", e))?;

        let representative = Account::decode_hex(&self.account)
            .map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(
            Wallets::new_null(&path).map_err(|e| anyhow!("Failed to create wallets: {:?}", e))?,
        );

        let password = self.password.clone().unwrap_or_default();

        wallets.ensure_wallet_is_unlocked(wallet_id, &password);

        wallets
            .set_representative(wallet_id, representative, false)
            .map_err(|e| anyhow!("Failed to set representative: {:?}", e))?;

        Ok(())
    }
}
