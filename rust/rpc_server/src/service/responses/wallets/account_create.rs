use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{AccountRpcMessage, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_create(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    index: Option<u32>,
    work: Option<bool>
) -> String {
    if enable_control {
        let work = work.unwrap_or(true);
        let result = if let Some(i) = index {
            node.wallets.deterministic_insert_at(&wallet, i, work)
        } else {
            node.wallets.deterministic_insert2(&wallet, work)
        };
        match result {
            Ok(account) => to_string_pretty(&AccountRpcMessage::new(
                "account".to_string(),
                account.as_account(),
            ))
            .unwrap(),
            Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
        }
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn account_create_options_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        let result = node
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None, None).await.unwrap() });

        assert!(node.wallets.exists(&result.value.into()));

        server.abort();
    }

    #[test]
    fn account_create_index_max() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        let result = node.tokio.block_on(async {
            rpc_client
                .account_create(wallet_id, Some(u32::MAX), None)
                .await
                .unwrap()
        });

        assert!(node.wallets.exists(&result.value.into()));

        server.abort();
    }

    #[test]
    fn account_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        let result = node
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None, None).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }

    #[test]
    fn account_create_fails_wallet_locked() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        node.wallets.lock(&wallet_id).unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None, None).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"Wallet is locked\"".to_string())
        );

        server.abort();
    }

    #[test]
    fn account_create_fails_wallet_not_found() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::random();

        let result = node
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None, None).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"Wallet not found\"".to_string())
        );

        server.abort();
    }
}
