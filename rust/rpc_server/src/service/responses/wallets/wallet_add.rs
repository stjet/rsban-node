use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{AccountDto, ErrorDto2, RpcDto, WalletAddArgs};
use std::sync::Arc;

pub async fn wallet_add(node: Arc<Node>, enable_control: bool, args: WalletAddArgs) -> RpcDto {
    if enable_control {
        let generate_work = args.work.unwrap_or(true);
        match node
            .wallets
            .insert_adhoc2(&args.wallet, &args.key, generate_work)
        {
            Ok(account) => RpcDto::Account(AccountDto::new(account.as_account())),
            Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e)),
        }
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
