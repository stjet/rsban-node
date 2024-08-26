use super::format_error_message;
use rsnano_core::WalletId;
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountListDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_list(node: Arc<Node>, wallet: WalletId) -> String {
    match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => {
            let account_list =
                AccountListDto::new(accounts.iter().map(|account| account.into()).collect());
            to_string_pretty(&account_list).unwrap()
        }
        Err(_) => format_error_message("Wallet not found"),
    }
}
