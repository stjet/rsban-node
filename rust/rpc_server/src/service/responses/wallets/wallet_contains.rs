use crate::service::responses::format_error_message;
use rsnano_core::{Account, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::ExistsDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_contains(node: Arc<Node>, wallet: WalletId, account: Account) -> String {
    let wallet_accounts = match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => accounts,
        Err(_) => return format_error_message("Failed to get accounts of wallet"),
    };

    let mut wallet_contains = ExistsDto::new(false);
    if wallet_accounts.contains(&account) {
        wallet_contains.exists = true;
    }

    to_string_pretty(&wallet_contains).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn wallet_contains_true() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet: WalletId = 1.into();

        node.wallets.create(1.into());

        let account = node
            .wallets
            .deterministic_insert2(&wallet, false)
            .unwrap()
            .into();

        assert!(node.wallets.exists(&account));

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_contains(wallet, account.into())
                .await
                .unwrap()
        });

        assert_eq!(result.exists, true);

        server.abort();
    }

    #[test]
    fn wallet_contains_false() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet: WalletId = 1.into();

        node.wallets.create(1.into());

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_contains(wallet, Account::zero())
                .await
                .unwrap()
        });

        assert_eq!(result.exists, false);

        server.abort();
    }
}
