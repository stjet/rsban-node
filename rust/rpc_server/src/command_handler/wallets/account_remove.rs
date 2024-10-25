use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RemovedDto, RpcDto, WalletWithAccountArgs};

impl RpcCommandHandler {
    pub(crate) fn account_remove(&self, args: WalletWithAccountArgs) -> RpcDto {
        if self.enable_control {
            match self
                .node
                .wallets
                .remove_key(&args.wallet, &args.account.into())
            {
                Ok(()) => RpcDto::Removed(RemovedDto::new(true)),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
