use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{SuccessResponse, WalletWithPasswordArgs};

impl RpcCommandHandler {
    pub(crate) fn password_change(
        &self,
        args: WalletWithPasswordArgs,
    ) -> anyhow::Result<SuccessResponse> {
        self.node.wallets.rekey(&args.wallet, args.password)?;
        Ok(SuccessResponse::new())
    }
}
