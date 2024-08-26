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

#[cfg(test)]
mod tests {
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::{create_wallet, setup_rpc_client_and_server, System};

    #[test]
    fn account_list() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

        let wallet = create_wallet(node.clone());

        let pk = node.wallets.deterministic_insert2(&wallet, false).unwrap();

        let result = node
            .async_rt
            .tokio
            .block_on(async { rpc_client.account_list(wallet).await.unwrap() });

        assert_eq!(vec![pk], result.accounts);

        server.abort();
    }
}
