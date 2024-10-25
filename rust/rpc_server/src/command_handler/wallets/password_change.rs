use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{SuccessDto, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_change(
        &self,
        args: WalletWithPasswordArgs,
    ) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node.wallets.rekey(&args.wallet, args.password)?;
        Ok(SuccessDto::new())
    }
}
