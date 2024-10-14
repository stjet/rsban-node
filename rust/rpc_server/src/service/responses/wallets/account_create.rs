use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{AccountCreateArgs, AccountDto, ErrorDto2};
use std::sync::Arc;
use crate::RpcResult;

pub async fn account_create(
    node: Arc<Node>,
    enable_control: bool,
    args: AccountCreateArgs
) -> RpcResult<AccountDto> {
    if !enable_control {
        return RpcResult::Err(ErrorDto2::RPCControlDisabled);
    }

    let work = args.work.unwrap_or(true);

    let result = match args.index {
        Some(i) => node.wallets.deterministic_insert_at(&args.wallet, i, work),
        None => node.wallets.deterministic_insert2(&args.wallet, work),
    };

    match result {
        Ok(account) => RpcResult::Ok(AccountDto::new(
            account.as_account(),
        )),
        Err(e) => RpcResult::Err(ErrorDto2::WalletsError(e)),
    }
}
