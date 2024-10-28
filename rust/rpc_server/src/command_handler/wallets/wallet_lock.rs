use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{LockedDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_lock(&self, args: WalletRpcMessage) -> anyhow::Result<LockedDto> {
        self.node.wallets.lock(&args.wallet)?;
        Ok(LockedDto::new(true))
    }
}
