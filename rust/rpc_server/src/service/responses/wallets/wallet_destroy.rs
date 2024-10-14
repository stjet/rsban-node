use rsnano_core::WalletId;
use rsnano_node::Node;
use rsnano_rpc_messages::{DestroyedDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_destroy(node: Arc<Node>, enable_control: bool, wallet: WalletId) -> String {
    if enable_control {
        node.wallets.destroy(&wallet);
        to_string_pretty(&DestroyedDto::new(true)).unwrap()
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}
