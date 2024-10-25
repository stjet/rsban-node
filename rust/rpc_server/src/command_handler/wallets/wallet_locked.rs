use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{LockedDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_locked(&self, args: WalletRpcMessage) -> anyhow::Result<LockedDto> {
        let valid = self.node.wallets.valid_password(&args.wallet)?;
        Ok(LockedDto::new(!valid))
    }
}
