use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountGetArgs {
    #[arg(long)]
    key: String,
}

impl AccountGetArgs {
    pub(crate) fn account_get(&self) {
        let account = Account::decode_hex(&self.key).unwrap();
        println!("Account: {:?}", account.encode_account());
    }
}
