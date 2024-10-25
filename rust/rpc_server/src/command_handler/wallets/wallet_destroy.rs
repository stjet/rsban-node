use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{DestroyedDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_destroy(&self, args: WalletRpcMessage) -> anyhow::Result<DestroyedDto> {
        self.ensure_control_enabled()?;
        self.node.wallets.destroy(&args.wallet);
        Ok(DestroyedDto::new(true))
    }
}
