use rsnano_core::{Account, KeyPair};
use rsnano_rpc_messages::KeyPairDto;

pub(crate) fn key_create() -> KeyPairDto {
    let keypair = KeyPair::new();
    let private = keypair.private_key();
    let public = keypair.public_key();
    let account = Account::from(public);
    KeyPairDto::new(private, public, account)
}
