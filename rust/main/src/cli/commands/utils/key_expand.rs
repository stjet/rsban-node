use anyhow::{anyhow, Result};
use clap::Parser;
use rsnano_core::{Account, PublicKey, RawKey};

#[derive(Parser)]
pub(crate) struct KeyExpandArgs {
    #[arg(long)]
    key: String,
}

impl KeyExpandArgs {
    pub(crate) fn key_expand(&self) -> Result<()> {
        let private_key =
            RawKey::decode_hex(&self.key).map_err(|e| anyhow!("Key is invalid: {:?}", e))?;
        let public_key =
            PublicKey::try_from(&private_key).expect("This should not fail since the key is valid");
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);

        Ok(())
    }
}
