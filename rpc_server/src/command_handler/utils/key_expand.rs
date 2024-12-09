use anyhow::anyhow;
use rsban_core::{Account, PublicKey};
use rsban_rpc_messages::{KeyExpandArgs, KeyPairDto};

pub fn key_expand(args: KeyExpandArgs) -> anyhow::Result<KeyPairDto> {
    let public: PublicKey = (&args.key)
        .try_into()
        .map_err(|_| anyhow!("Bad private key"))?;
    let account = Account::from(public);
    Ok(KeyPairDto::new(args.key, public, account))
}
