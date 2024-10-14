use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{AccountCreateArgs, AccountDto, ErrorDto2, RpcDto};
use std::sync::Arc;

pub async fn account_create(
    node: Arc<Node>,
    enable_control: bool,
    args: AccountCreateArgs,
) -> RpcDto {
    if !enable_control {
        return RpcDto::Error(ErrorDto2::RPCControlDisabled);
    }

    let work = args.work.unwrap_or(true);

    let result = match args.index {
        Some(i) => node.wallets.deterministic_insert_at(&args.wallet, i, work),
        None => node.wallets.deterministic_insert2(&args.wallet, work),
    };

    match result {
        Ok(account) => RpcDto::Account(AccountDto::new(account.as_account())),
        Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e)),
    }
}
