use rsnano_core::{Account, WalletId};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto};
use std::sync::Arc;

pub async fn wallet_add_watch(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    accounts: Vec<Account>,
) -> RpcDto {
    if enable_control {
        match node.wallets.insert_watch(&wallet, &accounts) {
            Ok(_) => RpcDto::WalletAddWatch(SuccessDto::new()),
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto::RPCControlDisabled)
    }
}
