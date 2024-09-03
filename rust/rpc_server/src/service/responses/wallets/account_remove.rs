use rsnano_core::{Account, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BoolDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_remove(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    account: Account,
) -> String {
    if enable_control {
        match node.wallets.remove_key(&wallet, &account.into()) {
            Ok(()) => to_string_pretty(&BoolDto::new("removed".to_string(), true)).unwrap(),
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
    fn account_remove() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        let account = node
            .wallets
            .deterministic_insert2(&wallet, false)
            .unwrap()
            .into();

        assert!(node.wallets.exists(&account));

        node.tokio.block_on(async {
            rpc_client
                .account_remove(wallet, account.into())
                .await
                .unwrap()
        });

        assert!(!node.wallets.exists(&account));

        server.abort();
    }

    #[test]
    fn account_remove_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet = WalletId::random();

        node.wallets.create(wallet);

        let account = node
            .wallets
            .deterministic_insert2(&wallet, false)
            .unwrap()
            .into();

        assert!(node.wallets.exists(&account));

        let result = node
            .tokio
            .block_on(async { rpc_client.account_remove(wallet, account.into()).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}
