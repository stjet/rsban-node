use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{AccountRpcMessage, ErrorDto, RpcDto, WalletAddArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_add(&self, args: WalletAddArgs) -> RpcDto {
        if self.enable_control {
            let generate_work = args.work.unwrap_or(true);
            match self
                .node
                .wallets
                .insert_adhoc2(&args.wallet, &args.key, generate_work)
            {
                Ok(account) => RpcDto::Account(AccountRpcMessage::new(account.as_account())),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
