use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::WalletId;
use rsnano_node::wallets::Wallets;
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletDestroyArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletDestroyArgs {
    pub(crate) fn wallet_destroy(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        match WalletId::decode_hex(&self.wallet) {
            Ok(wallet) => wallets.destroy(&wallet),
            Err(_) => println!("Invalid wallet id"),
        }
    }
}
