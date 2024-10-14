use rsnano_core::WalletId;
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{ErrorDto, ValidDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn password_enter(node: Arc<Node>, wallet: WalletId, password: String) -> String {
    match node.wallets.enter_password(wallet, &password) {
        Ok(_) => to_string_pretty(&ValidDto::new(true)).unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}
