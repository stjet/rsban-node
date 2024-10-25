use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{SuccessDto, WalletAddWatchArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_add_watch(&self, args: WalletAddWatchArgs) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node
            .wallets
            .insert_watch(&args.wallet, &args.accounts)?;
        Ok(SuccessDto::new())
    }
}
