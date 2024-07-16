use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletRepresentativeSetArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    account: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletRepresentativeSetArgs {
    pub(crate) fn wallet_representative_set(&self) {
        let wallet_id = WalletId::decode_hex(&self.wallet).unwrap();

        let representative = Account::decode_hex(&self.account).unwrap();

        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        wallets
            .set_representative(wallet_id, representative, false)
            .unwrap();
    }
}
