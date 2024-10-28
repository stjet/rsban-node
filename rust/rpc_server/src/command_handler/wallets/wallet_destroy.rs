use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{DestroyedDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_destroy(&self, args: WalletRpcMessage) -> DestroyedDto {
        self.node.wallets.destroy(&args.wallet);
        DestroyedDto::new(true)
    }
}
