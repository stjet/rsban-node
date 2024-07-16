use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountKeyArgs {
    #[arg(long)]
    account: String,
}

impl AccountKeyArgs {
    pub(crate) fn account_key(&self) {
        let key = Account::decode_account(&self.account).unwrap();
        println!("Hex: {:?}", key);
    }
}
