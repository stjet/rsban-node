use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{AccountsWithWorkDto, ErrorDto2, RpcDto};
use std::{collections::HashMap, sync::Arc};

pub async fn wallet_work_get(node: Arc<Node>, enable_control: bool, wallet: WalletId) -> RpcDto {
    if !enable_control {
        return RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }

    let accounts = match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => accounts,
        Err(e) => return RpcDto::Error(ErrorDto2::WalletsError(e))
    };

    let mut works = HashMap::new();

    for account in accounts {
        match node.wallets.work_get2(&wallet, &account.into()) {
            Ok(work) => {
                works.insert(account, work.into());
            }
            Err(e) => return RpcDto::Error(ErrorDto2::WalletsError(e))
        }
    }

    RpcDto::WalletWorkGet(AccountsWithWorkDto::new(works))
}
