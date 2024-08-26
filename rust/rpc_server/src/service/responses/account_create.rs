use crate::format_error_message;
use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountCreateDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_create(node: Arc<Node>, wallet: WalletId, index: Option<u32>) -> String {
    let result = if let Some(i) = index {
        node.wallets.deterministic_insert_at(&wallet, i, false)
    } else {
        node.wallets.deterministic_insert2(&wallet, false)
    };

    match result {
        Ok(account) => {
            println!("{:?}", account.as_account());
            to_string_pretty(&AccountCreateDto::new(account)).unwrap()
        }
        Err(_) => format_error_message("Wallet error"),
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};
    use rsnano_core::WalletId;
    use rsnano_node::{node::Node, wallets::WalletsExt};
    use test_helpers::{setup_rpc_client_and_server, System};

    fn create_wallet(node: &Node) -> WalletId {
        let wallet_id = WalletId::from_bytes(thread_rng().gen());
        node.wallets.create(wallet_id);
        wallet_id
    }

    #[test]
    fn account_create_index_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

        let wallet_id = create_wallet(&node);

        let result = node
            .async_rt
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

        let wallet_id = create_wallet(&node);

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_create(wallet_id, Some(u32::MAX))
                .await
                .unwrap()
        });

        assert!(node.wallets.exists(&result.account.into()));

        server.abort();
    }
}
