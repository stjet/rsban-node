use rsnano_core::{Account, WalletId};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, ExistsDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_contains(node: Arc<Node>, wallet: WalletId, account: Account) -> String {
    let wallet_accounts = match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => accounts,
        Err(e) => return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    };

    if wallet_accounts.contains(&account) {
        to_string_pretty(&ExistsDto::new(true)).unwrap()
    } else {
        to_string_pretty(&ExistsDto::new(false)).unwrap()
    }
}
