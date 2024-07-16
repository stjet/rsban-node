use std::sync::Arc;

use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::WalletId;
use rsnano_node::wallets::Wallets;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletRepresentativeGetArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletRepresentativeGetArgs {
    pub(crate) fn wallet_representative_get(&self) {
        let wallet_id = WalletId::decode_hex(&self.wallet).unwrap();

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        let representative = wallets.get_representative(wallet_id).unwrap();

        println!("Representative: {:?}", representative);
    }
}
