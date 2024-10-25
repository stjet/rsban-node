use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{AccountsWithWorkDto, ErrorDto, RpcDto, WalletRpcMessage};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn wallet_work_get(&self, args: WalletRpcMessage) -> RpcDto {
        if !self.enable_control {
            return RpcDto::Error(ErrorDto::RPCControlDisabled);
        }

        let accounts = match self.node.wallets.get_accounts_of_wallet(&args.wallet) {
            Ok(accounts) => accounts,
            Err(e) => return RpcDto::Error(ErrorDto::WalletsError(e)),
        };

        let mut works = HashMap::new();

        for account in accounts {
            match self.node.wallets.work_get2(&args.wallet, &account.into()) {
                Ok(work) => {
                    works.insert(account, work.into());
                }
                Err(e) => return RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        }

        RpcDto::WalletWorkGet(AccountsWithWorkDto::new(works))
    }
}
