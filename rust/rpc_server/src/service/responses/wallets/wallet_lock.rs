use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto2, LockDto, RpcDto, WalletLockArgs};
use std::sync::Arc;

pub async fn wallet_lock(node: Arc<Node>, enable_control: bool, args: WalletLockArgs) -> RpcDto {
    if enable_control {
        match node.wallets.lock(&args.wallet) {
            Ok(()) => RpcDto::Lock(LockDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
