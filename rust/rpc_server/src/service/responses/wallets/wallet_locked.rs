use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto2, LockedDto, RpcDto, WalletLockedArgs};
use std::sync::Arc;

pub async fn wallet_locked(node: Arc<Node>, args: WalletLockedArgs) -> RpcDto {
    match node.wallets.valid_password(&args.wallet) {
        Ok(valid) => RpcDto::Locked(LockedDto::new(!valid)),
        Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e)),
    }
}
