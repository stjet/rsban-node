use crate::command_handler::RpcCommandHandler;
use anyhow::bail;
use rsnano_core::{sign_message, utils::MemoryStream, BlockEnum, RawKey};
use rsnano_rpc_messages::{SignArgs, SignDto};

impl RpcCommandHandler {
    pub(crate) fn sign(&self, args: SignArgs) -> anyhow::Result<SignDto> {
        let block: BlockEnum = args.block.into();

        let prv = if let Some(key) = args.key {
            key
        } else if let (Some(wallet), Some(account)) = (args.wallet, args.account) {
            self.node.wallets.fetch(&wallet, &account.into())?
        } else {
            bail!(Self::MISSING_ACCOUNT_INFO);
        };

        let signature = if prv != RawKey::zero() {
            let mut stream = MemoryStream::new();
            block.serialize(&mut stream);
            let signature = sign_message(&prv, &stream.to_vec());
            signature
        } else {
            bail!(Self::MISSING_ACCOUNT_INFO);
        };

        Ok(SignDto::new(signature, block.json_representation()))
    }

    const MISSING_ACCOUNT_INFO: &str = "Missing account information";
}
