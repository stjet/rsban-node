use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, ValidDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn password_valid(&self, args: WalletRpcMessage) -> RpcDto {
        match self.node.wallets.valid_password(&args.wallet) {
            Ok(valid) => RpcDto::PasswordValid(ValidDto::new(valid)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
