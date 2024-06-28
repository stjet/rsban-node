use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountKeyOptions {
    #[arg(long)]
    account: String,
}

impl AccountKeyOptions {
    pub(crate) fn run(&self) {
        let key = Account::decode_account(&self.account).unwrap();
        println!("Hex: {:?}", key);
    }
}
