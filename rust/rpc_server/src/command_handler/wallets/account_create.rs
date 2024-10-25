use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{AccountCreateArgs, AccountRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn account_create(
        &self,
        args: AccountCreateArgs,
    ) -> anyhow::Result<AccountRpcMessage> {
        self.ensure_control_enabled()?;

        let work = args.work.unwrap_or(true);

        let account = match args.index {
            Some(i) => self
                .node
                .wallets
                .deterministic_insert_at(&args.wallet, i, work)?,
            None => self
                .node
                .wallets
                .deterministic_insert2(&args.wallet, work)?,
        };

        Ok(AccountRpcMessage::new(account.as_account()))
    }
}
