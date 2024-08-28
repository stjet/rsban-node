use crate::service::responses::format_error_message;
use rsnano_core::{Account, PublicKey, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountMovedDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_move(
    node: Arc<Node>,
    wallet: WalletId,
    source: WalletId,
    accounts: Vec<Account>,
) -> String {
    let public_keys: Vec<PublicKey> = accounts.iter().map(|account| account.into()).collect();
    let result = node.wallets.move_accounts(&source, &wallet, &public_keys);

    match result {
        Ok(_) => to_string_pretty(&AccountMovedDto::new(true)).unwrap(),
        Err(e) => format_error_message(&e.to_string()),
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

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

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

        assert_eq!(result.moved, true);

        let new_wallet_accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
        let new_source_accounts = node.wallets.get_accounts_of_wallet(&source).unwrap();

        assert!(new_wallet_accounts.contains(&account));
        assert!(!new_source_accounts.contains(&account));

        server.abort();
    }
}

// todo: test enable control
