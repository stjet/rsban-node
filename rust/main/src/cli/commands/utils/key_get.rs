use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct KeyGetArgs {
    #[arg(long)]
    account: String,
}

impl KeyGetArgs {
    pub(crate) fn account_key(&self) -> Result<()> {
        let key = Account::decode_account(&self.account)
            .map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        println!("Hex: {:?}", key);

        Ok(())
    }
}
