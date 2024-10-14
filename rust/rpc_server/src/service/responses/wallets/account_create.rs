use rsnano_core::WalletId;
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{AccountDto, ErrorDto2};
use std::sync::Arc;
use crate::RpcResult;

pub async fn account_create(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    index: Option<u32>,
    work: Option<bool>,
) -> RpcResult<AccountDto> {
    if !enable_control {
        return RpcResult::Err(ErrorDto2::RPCControlDisabled);
    }

    let work = work.unwrap_or(true);

    let result = match index {
        Some(i) => node.wallets.deterministic_insert_at(&wallet, i, work),
        None => node.wallets.deterministic_insert2(&wallet, work),
    };

    match result {
        Ok(account) => RpcResult::Ok(AccountDto::new(
            account.as_account(),
        )),
        Err(e) => RpcResult::Err(ErrorDto2::WalletsError(e)),
    }
}
