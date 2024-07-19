use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct AccountGetArgs {
    #[arg(long)]
    key: String,
}

impl AccountGetArgs {
    pub(crate) fn account_get(&self) -> Result<()> {
        let account =
            Account::decode_hex(&self.key).map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        println!("Account: {:?}", account.encode_account());

        Ok(())
    }
}
