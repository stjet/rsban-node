use clap::Parser;
use rsnano_core::{Account, PublicKey, RawKey};

#[derive(Parser)]
pub(crate) struct KeyExpandArgs {
    #[arg(long)]
    key: String,
}

impl KeyExpandArgs {
    pub(crate) fn key_expand(&self) {
        let private_key = RawKey::decode_hex(&self.key).unwrap();
        let public_key = PublicKey::try_from(&private_key).unwrap();
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);
    }
}
