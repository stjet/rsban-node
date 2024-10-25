use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto, WalletAddWatchArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_add_watch(&self, args: WalletAddWatchArgs) -> RpcDto {
        if self.enable_control {
            match self.node.wallets.insert_watch(&args.wallet, &args.accounts) {
                Ok(_) => RpcDto::WalletAddWatch(SuccessDto::new()),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
