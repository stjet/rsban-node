use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rand::{thread_rng, Rng};
use rsnano_core::{RawKey, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletCreateArgs {
    #[arg(long)]
    seed: Option<String>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletCreateArgs {
    pub(crate) fn wallet_create(&self) -> anyhow::Result<()> {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallet_id = WalletId::from_bytes(thread_rng().gen());
        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        wallets.create(wallet_id);
        println!("{:?}", wallet_id);

        if let Some(password) = &self.password {
            wallets.enter_password(wallet_id, password).unwrap();
        } else if let Some(seed) = &self.seed {
            match RawKey::decode_hex(seed) {
                Ok(key) => {
                    wallets.change_seed(wallet_id, &key, 0).unwrap();
                }
                Err(_) => println!("Invalid seed"),
            }
        }

        Ok(())
    }
}
