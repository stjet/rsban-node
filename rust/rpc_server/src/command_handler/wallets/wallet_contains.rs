use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, ExistsDto, RpcDto, WalletWithAccountArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_contains(&self, args: WalletWithAccountArgs) -> RpcDto {
        let wallet_accounts = match self.node.wallets.get_accounts_of_wallet(&args.wallet) {
            Ok(accounts) => accounts,
            Err(e) => return RpcDto::Error(ErrorDto::WalletsError(e)),
        };

        if wallet_accounts.contains(&args.account) {
            RpcDto::Exists(ExistsDto::new(true))
        } else {
            RpcDto::Exists(ExistsDto::new(false))
        }
    }
}
