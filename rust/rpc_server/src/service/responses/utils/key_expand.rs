use rsnano_core::{Account, PublicKey, RawKey};
use rsnano_rpc_messages::{KeyPairDto, RpcDto};

pub async fn key_expand(key: RawKey) -> RpcDto {
    let public: PublicKey = (&key).try_into().unwrap();
    let account = Account::from(public);

    RpcDto::KeyExpand(KeyPairDto::new(key, public, account))
}
