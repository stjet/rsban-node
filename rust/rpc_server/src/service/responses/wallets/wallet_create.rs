use crate::service::responses::format_error_message;
use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::WalletDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_create(node: Arc<Node>, enable_control: bool) -> String {
    if enable_control {
        let wallet = WalletId::random();

        node.wallets.create(wallet);

        to_string_pretty(&WalletDto::new(wallet)).unwrap()
    } else {
        format_error_message("RPC control is disabled")
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn wallet_create() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create().await.unwrap() });

        let wallets = node.wallets.wallet_ids();

        assert!(wallets.contains(&result.wallet));

        server.abort();
    }

    #[test]
    fn wallet_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create().await });

        assert!(result.is_err());

        server.abort();
    }
}
