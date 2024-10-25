use rsnano_core::{Account, PublicKey};
use rsnano_rpc_messages::{KeyExpandArgs, KeyPairDto};

pub fn key_expand(args: KeyExpandArgs) -> KeyPairDto {
    let public: PublicKey = (&args.key).try_into().unwrap();
    let account = Account::from(public);
    KeyPairDto::new(args.key, public, account)
}
