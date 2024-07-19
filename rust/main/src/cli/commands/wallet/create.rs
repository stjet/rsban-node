use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rand::{thread_rng, Rng};
use rsnano_core::{RawKey, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct CreateArgs {
    #[arg(long)]
    seed: Option<String>,
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

        let wallet_id = WalletId::from_bytes(thread_rng().gen());

        let wallets = Arc::new(
            Wallets::new_null(&path).map_err(|e| anyhow!("Failed to create wallets: {:?}", e))?,
        );

        wallets.create(wallet_id);

        println!("{:?}", wallet_id);

        let password = self.password.clone().unwrap_or_default();

        wallets
            .enter_password(wallet_id, &password)
            .map_err(|e| anyhow!("Failed to enter password: {:?}", e))?;

        if let Some(seed) = &self.seed {
            let key = RawKey::decode_hex(seed)
                .map_err(|e| anyhow!("Failed to enter password: {:?}", e))?;

            wallets
                .change_seed(wallet_id, &key, 0)
                .map_err(|e| anyhow!("Failed to change seed: {:?}", e))?;
        }

        Ok(())
    }
}
