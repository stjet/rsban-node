use rsnano_core::{Account, WalletId, WorkNonce};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto2, RpcDto, SuccessDto};
use std::sync::Arc;

pub async fn work_set(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    account: Account,
    work: WorkNonce,
) -> RpcDto {
    if enable_control {
        match node.wallets.work_set(&wallet, &account.into(), work.into()) {
            Ok(_) => RpcDto::WorkSet(SuccessDto::new()),
            Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
