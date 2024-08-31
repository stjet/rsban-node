use rsnano_core::{Account, PublicKey, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BoolDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_move(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    source: WalletId,
    accounts: Vec<Account>,
) -> String {
    if enable_control {
        let public_keys: Vec<PublicKey> = accounts.iter().map(|account| account.into()).collect();
        let result = node.wallets.move_accounts(&source, &wallet, &public_keys);

        match result {
            Ok(()) => to_string_pretty(&BoolDto::new("moved".to_string(), true)).unwrap(),
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
        let source = WalletId::random();

        node.wallets.create(wallet);
        node.wallets.create(source);

        let account = node
            .wallets
            .deterministic_insert2(&source, false)
            .unwrap()
            .into();

        let wallet_accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
        let source_accounts = node.wallets.get_accounts_of_wallet(&source).unwrap();

        assert!(!wallet_accounts.contains(&account));
        assert!(source_accounts.contains(&account));

        let result = node.tokio.block_on(async {
            rpc_client
                .account_move(wallet, source, vec![account])
                .await
                .unwrap()
        });

        assert_eq!(result.get("moved").unwrap(), true);

        let new_wallet_accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
        let new_source_accounts = node.wallets.get_accounts_of_wallet(&source).unwrap();

        assert!(new_wallet_accounts.contains(&account));
        assert!(!new_source_accounts.contains(&account));

        server.abort();
    }

    #[test]
    fn account_remove_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet = WalletId::random();
        let source = WalletId::random();

        node.wallets.create(wallet);
        node.wallets.create(source);

        let account = node
            .wallets
            .deterministic_insert2(&source, false)
            .unwrap()
            .into();

        let wallet_accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
        let source_accounts = node.wallets.get_accounts_of_wallet(&source).unwrap();

        assert!(!wallet_accounts.contains(&account));
        assert!(source_accounts.contains(&account));

        let result = node
            .tokio
            .block_on(async { rpc_client.account_move(wallet, source, vec![account]).await });

        assert!(result.is_err());

        server.abort();
    }
}
