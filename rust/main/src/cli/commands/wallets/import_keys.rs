use crate::cli::get_path;
use anyhow::Context;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_core::WalletId;
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::{fs::File, io::Read, path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct ImportKeysArgs {
    #[arg(long)]
    file: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    wallet: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl ImportKeysArgs {
    pub(crate) fn import_keys(&self) -> Result<()> {
        let mut file = File::open(PathBuf::from(&self.file))?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)
            .context("Unable to read <file> contents")?;

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallet_id = WalletId::decode_hex(&self.wallet)?;

        let wallets = Arc::new(Wallets::new_null(&path)?);

        let password = self.password.clone().unwrap_or_default();

        wallets.ensure_wallet_is_unlocked(wallet_id, &password);

        if wallets.wallet_exists(&wallet_id) {
            let valid = wallets.ensure_wallet_is_unlocked(wallet_id, &password);
            if valid {
                wallets.import_replace(wallet_id, &contents, &password)?
            } else {
                eprintln!("Invalid password for wallet {}. New wallet should have empty (default) password or passwords for new wallet & json file should match", wallet_id);
                return Err(anyhow!("Invalid arguments"));
            }
        } else {
            if !self.force {
                eprintln!("Wallet doesn't exist");
                return Err(anyhow!("Invalid arguments"));
            } else {
                wallets.import(wallet_id, &contents)?
            }
        }

        Ok(())
    }
}
