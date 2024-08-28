use crate::service::responses::format_error_message;
use rsnano_core::{Account, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountsCreatedDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn accounts_create(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    count: u64,
) -> String {
    if enable_control {
        let mut accounts: Vec<Account> = vec![];
        for _ in 0..count as usize {
            let account = node
                .wallets
                .deterministic_insert2(&wallet, false)
                .unwrap()
                .into();
            accounts.push(account)
        }
        to_string_pretty(&AccountsCreatedDto::new(accounts)).unwrap()
    } else {
        format_error_message("RPC control is disabled")
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn accounts_create() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        node.tokio
            .block_on(async { rpc_client.accounts_create(wallet, 8).await.unwrap() });

        assert_eq!(
            node.wallets.get_accounts_of_wallet(&wallet).unwrap().len(),
            8
        );

        server.abort();
    }

    #[test]
    fn accounts_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        let result = node
            .tokio
            .block_on(async { rpc_client.accounts_create(wallet, 8).await });

        assert!(result.is_err());

        server.abort();
    }
}
