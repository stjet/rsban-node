use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountGetOptions {
    #[arg(long)]
    key: String,
}

impl AccountGetOptions {
    pub(crate) fn run(&self) {
        let account = Account::decode_hex(&self.key).unwrap();
        println!("Account: {:?}", account.encode_account());
    }
}
