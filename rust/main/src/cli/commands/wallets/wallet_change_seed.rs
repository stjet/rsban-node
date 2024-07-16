use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{RawKey, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletChangeSeedArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    seed: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletChangeSeedArgs {
    pub(crate) fn wallet_change_seed(&self) {
        let wallet_id = WalletId::decode_hex(&self.wallet).unwrap();

        let seed = RawKey::decode_hex(&self.seed).unwrap();

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        wallets.change_seed(wallet_id, &seed, 0).unwrap();
    }
}
