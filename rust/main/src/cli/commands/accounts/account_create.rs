use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, WalletId};
use rsnano_node::wallets::{Wallets, WalletsExt};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct AccountCreateArgs {
    #[arg(long)]
    wallet: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl AccountCreateArgs {
    pub(crate) fn account_create(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());
        let wallet = WalletId::decode_hex(&self.wallet).unwrap();

        if self.password.is_none() {
            wallets.ensure_wallet_is_unlocked(wallet, "");
        }

        let public_key = wallets.deterministic_insert2(&wallet, false).unwrap();
        println!("Account: {:?}", Account::encode_account(&public_key));
    }
}
