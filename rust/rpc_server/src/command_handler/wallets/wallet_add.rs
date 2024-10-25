use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{AccountResponse, WalletAddArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_add(&self, args: WalletAddArgs) -> anyhow::Result<AccountResponse> {
        self.ensure_control_enabled()?;
        let generate_work = args.work.unwrap_or(true);
        let account = self
            .node
            .wallets
            .insert_adhoc2(&args.wallet, &args.key, generate_work)?;
        Ok(AccountResponse::new(account.as_account()))
    }
}
