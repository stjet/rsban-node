use rsnano_core::WalletId;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BoolDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn password_valid(node: Arc<Node>, wallet: WalletId) -> String {
    match node.wallets.valid_password(&wallet) {
        Ok(valid) => to_string_pretty(&BoolDto::new("valid".to_string(), valid)).unwrap(),
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
    fn password_valid() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        let result = node
            .tokio
            .block_on(async { rpc_client.password_valid(wallet_id).await.unwrap() });

        assert_eq!(result.value, true);

        server.abort();
    }

    #[test]
    fn password_invalid() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        node.wallets
            .rekey(&wallet_id, "password".to_string())
            .unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.password_valid(wallet_id).await.unwrap() });

        assert_eq!(result.value, true);

        server.abort();
    }
}
