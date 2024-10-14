use rsnano_node::Node;
use rsnano_rpc_messages::{AccountListArgs, AccountsDto, ErrorDto2};
use std::sync::Arc;
use crate::RpcResult;

pub async fn account_list(node: Arc<Node>, args: AccountListArgs) -> RpcResult<AccountsDto> {
    match node.wallets.get_accounts_of_wallet(&args.wallet) {
        Ok(accounts) => RpcResult::Ok(AccountsDto::new(accounts)),
        Err(e) => RpcResult::Err(ErrorDto2::WalletsError(e))
    }
}
