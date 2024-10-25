use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, LockedDto, RpcDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_lock(&self, args: WalletRpcMessage) -> RpcDto {
        if self.enable_control {
            match self.node.wallets.lock(&args.wallet) {
                Ok(()) => RpcDto::Lock(LockedDto::new(true)),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
