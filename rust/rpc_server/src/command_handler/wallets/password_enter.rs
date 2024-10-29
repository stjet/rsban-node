use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::{WalletsError, WalletsExt};
use rsnano_rpc_messages::{ValidDto, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_enter(&self, args: WalletWithPasswordArgs) -> anyhow::Result<ValidDto> {
        match self
            .node
            .wallets
            .enter_password(args.wallet, &args.password)
        {
            Ok(_) => Ok(ValidDto::new(true)),
            Err(WalletsError::InvalidPassword) => Ok(ValidDto::new(false)),
            Err(e) => Err(e.into()),
        }
    }
}
