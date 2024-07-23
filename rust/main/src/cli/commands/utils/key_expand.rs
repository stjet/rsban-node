use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::{Account, PublicKey, RawKey};

#[derive(Parser)]
pub(crate) struct ExpandKeyArgs {
    /// Derives the public key and the account from <key>
    #[arg(long)]
    key: String,
}

impl ExpandKeyArgs {
    pub(crate) fn expand_key(&self) -> Result<()> {
        let private_key = RawKey::decode_hex(&self.key)?;
        let public_key = PublicKey::try_from(&private_key)?;
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);

        Ok(())
    }
}
