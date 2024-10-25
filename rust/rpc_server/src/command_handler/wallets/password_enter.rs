use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ErrorDto, RpcDto, ValidDto, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_enter(&self, args: WalletWithPasswordArgs) -> RpcDto {
        match self
            .node
            .wallets
            .enter_password(args.wallet, &args.password)
        {
            Ok(_) => RpcDto::PasswordEnter(ValidDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
