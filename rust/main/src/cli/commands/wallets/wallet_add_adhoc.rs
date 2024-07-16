use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{RawKey, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletAddAdhocArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    key: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletAddAdhocArgs {
    pub(crate) fn wallet_add_adhoc(&self) {
        let wallet_id = WalletId::decode_hex(&self.wallet).unwrap();

        let key = RawKey::decode_hex(&self.key).unwrap();

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        wallets.insert_adhoc2(&wallet_id, &key, false).unwrap();
    }
}
