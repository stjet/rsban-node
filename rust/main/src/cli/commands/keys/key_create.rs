use clap::Parser;
use rsnano_core::{Account, KeyPair};

#[derive(Parser)]
pub(crate) struct KeyCreateOptions;

impl KeyCreateOptions {
    pub(crate) fn run(&self) {
        let keypair = KeyPair::new();
        let private_key = keypair.private_key();
        let public_key = keypair.public_key();
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);
    }
}
