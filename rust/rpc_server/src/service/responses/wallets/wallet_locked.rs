use crate::service::responses::{format_bool_message, format_error_message};
use rsnano_core::WalletId;
use rsnano_node::node::Node;
use std::sync::Arc;

pub async fn wallet_locked(node: Arc<Node>, wallet: WalletId) -> String {
    match node.wallets.valid_password(&wallet) {
        Ok(valid) => format_bool_message("locked", !valid),
        Err(_) => format_error_message("Wallet error"),
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn wallet_locked_false() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        assert_eq!(node.wallets.valid_password(&wallet_id).unwrap(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_locked(wallet_id).await.unwrap() });

        assert_eq!(result.get("locked").unwrap(), false);

        server.abort();
    }

    #[test]
    fn wallet_locked_true() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        node.wallets.lock(&wallet_id).unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_locked(wallet_id).await.unwrap() });

        assert_eq!(result.get("locked").unwrap(), true);

        server.abort();
    }
}
