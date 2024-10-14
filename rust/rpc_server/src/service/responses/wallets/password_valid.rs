use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, ValidDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn password_valid(node: Arc<Node>, wallet: WalletId) -> String {
    match node.wallets.valid_password(&wallet) {
        Ok(valid) => to_string_pretty(&ValidDto::new(valid)).unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}
