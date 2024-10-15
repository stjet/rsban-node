use rsnano_core::{Account, WalletId};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, WorkDto};
use std::sync::Arc;

pub async fn work_get(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    account: Account,
) -> RpcDto {
    if enable_control {
        match node.wallets.work_get2(&wallet, &account.into()) {
            Ok(work) => RpcDto::WorkGet(WorkDto::new(work.into())),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
