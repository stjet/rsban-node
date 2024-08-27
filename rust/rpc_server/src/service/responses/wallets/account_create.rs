use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountCreatedDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

use crate::service::responses::format_error_message;

pub async fn account_create(node: Arc<Node>, wallet: WalletId, index: Option<u32>) -> String {
    let result = if let Some(i) = index {
        node.wallets.deterministic_insert_at(&wallet, i, false)
    } else {
        node.wallets.deterministic_insert2(&wallet, false)
    };

    match result {
        Ok(account) => to_string_pretty(&AccountCreatedDto::new(account.as_account())).unwrap(),
        Err(_) => format_error_message("Wallet error"),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{create_wallet, setup_rpc_client_and_server};
    use test_helpers::System;

    #[test]
    fn account_create_index_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

        let wallet_id = create_wallet(node.clone());

        let result = node
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None).await.unwrap() });

        assert!(node.wallets.exists(&result.account.into()));

        server.abort();
    }

    #[test]
    fn account_create_index_max() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

        let wallet_id = create_wallet(node.clone());

        let result = node.tokio.block_on(async {
            rpc_client
                .account_create(wallet_id, Some(u32::MAX))
                .await
                .unwrap()
        });

        assert!(node.wallets.exists(&result.account.into()));

        server.abort();
    }
}
