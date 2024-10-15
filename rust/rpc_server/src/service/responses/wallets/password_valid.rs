use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto2, RpcDto, ValidDto};
use std::sync::Arc;

pub async fn password_valid(node: Arc<Node>, wallet: WalletId) -> RpcDto {
    match node.wallets.valid_password(&wallet) {
        Ok(valid) => RpcDto::PasswordValid(ValidDto::new(valid)),
        Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e))
    }
}
