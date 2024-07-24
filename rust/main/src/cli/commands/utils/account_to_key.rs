use anyhow::Result;
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountToKeyArgs {
    /// Converts the <account> into key
    #[arg(long)]
    account: String,
}

impl AccountToKeyArgs {
    pub(crate) fn account_to_key(&self) -> Result<()> {
        let key = Account::decode_account(&self.account)?;

        println!("Hex: {:?}", key);

        Ok(())
    }
}
