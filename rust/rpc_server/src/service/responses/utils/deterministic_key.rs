use rsnano_core::{Account, PublicKey, RawKey};
use rsnano_rpc_messages::{KeyPairDto, RpcDto};

pub async fn deterministic_key(seed: RawKey, index: u32) -> RpcDto {
    let private = rsnano_core::deterministic_key(&seed, index);
    let public: PublicKey = (&private).try_into().unwrap();
    let account = Account::from(public);

    RpcDto::DeterministicKey(KeyPairDto::new(private, public, account))
}
