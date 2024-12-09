use rsban_core::{Account, PublicKey};
use rsban_rpc_messages::{DeterministicKeyArgs, KeyPairDto};

pub fn deterministic_key(args: DeterministicKeyArgs) -> KeyPairDto {
    let private = rsban_core::deterministic_key(&args.seed, args.index.inner());
    let public: PublicKey = (&private).try_into().unwrap();
    let account = Account::from(public);
    KeyPairDto::new(private, public, account)
}
