use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto2, LockedDto, RpcDto, WalletRpcMessage};
use std::sync::Arc;

pub async fn wallet_lock(node: Arc<Node>, enable_control: bool, args: WalletRpcMessage) -> RpcDto {
    if enable_control {
        match node.wallets.lock(&args.wallet) {
            Ok(()) => RpcDto::Lock(LockedDto::new(true)),
            Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e))
        }
    } else {
        RpcDto::Error(ErrorDto2::RPCControlDisabled)
    }
}
