use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct RemoveArgs {
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

impl RemoveArgs {
    pub(crate) fn wallet_remove(&self) -> Result<()> {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(
            Wallets::new_null(&path).map_err(|e| anyhow!("Failed to create wallets: {:?}", e))?,
        );

        let wallet_id = WalletId::decode_hex(&self.wallet)?;

        let password = self.password.clone().unwrap_or_default();

        wallets.ensure_wallet_is_unlocked(wallet_id, &password);

        let account = Account::decode_hex(&self.account)
            .map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        wallets
            .remove_account(&wallet_id, &account)
            .map_err(|e| anyhow!("Failed to remove account: {:?}", e))?;

        Ok(())
    }
}
