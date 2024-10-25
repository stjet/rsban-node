use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ExistsDto, WalletWithAccountArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_contains(&self, args: WalletWithAccountArgs) -> anyhow::Result<ExistsDto> {
        let wallet_accounts = self.node.wallets.get_accounts_of_wallet(&args.wallet)?;
        let exists = wallet_accounts.contains(&args.account);
        Ok(ExistsDto::new(exists))
    }
}
