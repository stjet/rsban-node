use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{DestroyedDto, ErrorDto, RpcDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_destroy(&self, args: WalletRpcMessage) -> RpcDto {
        if self.enable_control {
            self.node.wallets.destroy(&args.wallet);
            RpcDto::Destroyed(DestroyedDto::new(true))
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
