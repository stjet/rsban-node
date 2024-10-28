use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{WalletWithAccountArgs, WorkDto};

impl RpcCommandHandler {
    pub(crate) fn work_get(&self, args: WalletWithAccountArgs) -> anyhow::Result<WorkDto> {
        let work = self
            .node
            .wallets
            .work_get2(&args.wallet, &args.account.into())?;
        Ok(WorkDto::new(work.into()))
    }
}
