use crate::cli::get_path;
use clap::Parser;
use rsnano_core::WalletId;
use rsnano_node::wallets::Wallets;
use std::sync::Arc;

#[derive(Parser)]
#[command(group = clap::ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletDecryptUnsafeOptions {
    #[arg(long)]
    wallet: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletDecryptUnsafeOptions {
    pub(crate) fn run(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        match WalletId::decode_hex(&self.wallet) {
            Ok(wallet) => {
                let seed = wallets.get_seed(wallet).unwrap();
                println!("Seed: {:?}", seed);
            }
            Err(_) => println!("Invalid wallet id"),
        }
    }
}
