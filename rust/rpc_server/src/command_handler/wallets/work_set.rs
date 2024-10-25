use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{SuccessDto, WorkSetArgs};

impl RpcCommandHandler {
    pub(crate) fn work_set(&self, args: WorkSetArgs) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node
            .wallets
            .work_set(&args.wallet, &args.account.into(), args.work.into())?;
        Ok(SuccessDto::new())
    }
}
