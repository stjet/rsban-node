use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::Account;

#[derive(Parser)]
pub(crate) struct GetArgs {
    #[arg(long)]
    key: String,
}

impl GetArgs {
    pub(crate) fn get(&self) -> Result<()> {
        let account =
            Account::decode_hex(&self.key).map_err(|e| anyhow!("Account is invalid: {:?}", e))?;

        println!("Account: {:?}", account.encode_account());

        Ok(())
    }
}
