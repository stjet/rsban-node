use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountCreateResponse;
use serde_json::to_string_pretty;
use std::sync::Arc;

use super::format_error_message;

pub async fn account_create(node: Arc<Node>, wallet: WalletId, index: Option<u32>) -> String {
    let result = if let Some(i) = index {
        node.wallets.deterministic_insert_at(&wallet, i, false)
    } else {
        node.wallets.deterministic_insert2(&wallet, false)
    };

    match result {
        Ok(account) => to_string_pretty(&AccountCreateResponse::new(account.as_account())).unwrap(),
        Err(e) => format_error_message("Wallet error"),
    }
}
