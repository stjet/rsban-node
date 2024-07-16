use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::Wallets;
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletRemoveArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    account: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletRemoveArgs {
    pub(crate) fn wallet_remove(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        match WalletId::decode_hex(&self.wallet) {
            Ok(wallet) => match Account::decode_hex(&self.account) {
                Ok(account) => wallets.remove_account(&wallet, &account).unwrap(),
                Err(_) => println!("Invalid account id"),
            },
            Err(_) => println!("Invalid wallet id"),
        }
    }
}
