use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct KeyToAccountArgs {
    /// Converts the <key> into account
    #[arg(long)]
    key: String,
}

impl KeyToAccountArgs {
    pub(crate) fn key_to_account(&self) -> Result<()> {
        let account = Account::decode_hex(&self.key)?;

        println!("Account: {:?}", account.encode_account());

        Ok(())
    }
}
