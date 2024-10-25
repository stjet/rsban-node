use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ValidDto, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_enter(&self, args: WalletWithPasswordArgs) -> anyhow::Result<ValidDto> {
        self.node
            .wallets
            .enter_password(args.wallet, &args.password)?;
        Ok(ValidDto::new(true))
    }
}
