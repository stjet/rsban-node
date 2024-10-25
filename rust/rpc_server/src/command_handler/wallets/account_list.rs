use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountsRpcMessage, ErrorDto, RpcDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn account_list(&self, args: WalletRpcMessage) -> RpcDto {
        match self.node.wallets.get_accounts_of_wallet(&args.wallet) {
            Ok(accounts) => RpcDto::Accounts(AccountsRpcMessage::new(accounts)),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
