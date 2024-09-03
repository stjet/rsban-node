use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{BoolDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn password_enter(node: Arc<Node>, wallet: WalletId, password: String) -> String {
    match node.wallets.enter_password(wallet, &password) {
        Ok(_) => to_string_pretty(&BoolDto::new("valid".to_string(), true)).unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn password_change() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        node.tokio.block_on(async {
            rpc_client
                .password_enter(wallet_id, "".to_string())
                .await
                .unwrap()
        });

        server.abort();
    }

    #[test]
    fn password_change_fails_with_invalid_password() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        let result = node.tokio.block_on(async {
            rpc_client
                .password_enter(wallet_id, "password".to_string())
                .await
        });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"Invalid password\"".to_string())
        );

        server.abort();
    }
}
