use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{AccountCreateArgs, AccountRpcMessage, ErrorDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn account_create(&self, args: AccountCreateArgs) -> RpcDto {
        if !self.enable_control {
            return RpcDto::Error(ErrorDto::RPCControlDisabled);
        }

        let work = args.work.unwrap_or(true);

        let result = match args.index {
            Some(i) => self
                .node
                .wallets
                .deterministic_insert_at(&args.wallet, i, work),
            None => self.node.wallets.deterministic_insert2(&args.wallet, work),
        };

        match result {
            Ok(account) => RpcDto::Account(AccountRpcMessage::new(account.as_account())),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
