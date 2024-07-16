use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::Account;
use rsnano_node::wallets::Wallets;
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletListArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl WalletListArgs {
    pub(crate) fn wallet_list(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        let wallet_ids = wallets.get_wallet_ids();

        for wallet_id in wallet_ids {
            println!("{:?}", wallet_id);
            let accounts = wallets.get_accounts_of_wallet(&wallet_id).unwrap();
            if !accounts.is_empty() {
                for account in accounts {
                    println!("{:?}", Account::encode_account(&account));
                }
            }
        }
    }
}
