use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, LockedDto, RpcDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_locked(&self, args: WalletRpcMessage) -> RpcDto {
        match self.node.wallets.valid_password(&args.wallet) {
            Ok(valid) => RpcDto::Locked(LockedDto::new(!valid)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
