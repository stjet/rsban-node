use rsnano_core::{Account, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountsCreatedDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn accounts_create(node: Arc<Node>, wallet: WalletId, count: u64) -> String {
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

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

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
}
