use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct KeyToAccountArgs {
    /// Get account number for the <key>
    #[arg(long)]
    key: String,
}

impl KeyToAccountArgs {
    pub(crate) fn key_to_account(&self) -> Result<()> {
        let account =
            Account::decode_hex(&self.key).map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        println!("Account: {:?}", account.encode_account());

        Ok(())
    }
}
