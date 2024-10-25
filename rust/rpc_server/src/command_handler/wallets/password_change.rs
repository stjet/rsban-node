use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_change(&self, args: WalletWithPasswordArgs) -> RpcDto {
        if self.enable_control {
            match self.node.wallets.rekey(&args.wallet, args.password) {
                Ok(_) => RpcDto::PasswordChange(SuccessDto::new()),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
