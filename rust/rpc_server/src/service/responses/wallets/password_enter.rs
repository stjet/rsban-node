use rsnano_core::WalletId;
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{ErrorDto2, RpcDto, ValidDto};
use std::sync::Arc;

pub async fn password_enter(node: Arc<Node>, wallet: WalletId, password: String) -> RpcDto {
    match node.wallets.enter_password(wallet, &password) {
        Ok(_) => RpcDto::PasswordEnter(ValidDto::new(true)),
        Err(e) => RpcDto::Error(ErrorDto2::WalletsError(e)),
    }
}
