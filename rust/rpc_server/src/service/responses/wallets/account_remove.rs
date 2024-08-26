use rsnano_core::{Account, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountRemoveDto;
use std::sync::Arc;
use toml::to_string_pretty;

pub async fn account_remove(node: Arc<Node>, wallet: WalletId, account: Account) -> String {
    let mut account_remove = AccountRemoveDto::new(false);
    if node.wallets.remove_key(&wallet, &account.into()).is_ok() {
        account_remove.removed = true;
    }
    to_string_pretty(&account_remove).unwrap()
}
