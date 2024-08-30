use crate::service::responses::{format_bool_message, format_error_message};
use rsnano_core::WalletId;
use rsnano_node::node::Node;
use std::sync::Arc;

pub async fn wallet_lock(node: Arc<Node>, enable_control: bool, wallet: WalletId) -> String {
    if enable_control {
        match node.wallets.lock(&wallet) {
            Ok(()) => format_bool_message("locked", true),
            Err(_) => format_error_message("Failed to lock wallet"),
        }
    } else {
        format_error_message("RPC control is disabled")
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use rsnano_rpc_messages::RpcCommand;
    use test_helpers::System;

    #[test]
    fn wallet_lock() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        assert_eq!(node.wallets.valid_password(&wallet_id).unwrap(), true);

        node.tokio.block_on(async {
            rpc_client
                .rpc_request(&RpcCommand::wallet_lock(wallet_id))
                .await
                .unwrap()
        });

        assert_eq!(node.wallets.valid_password(&wallet_id).unwrap(), false);

        server.abort();
    }

    #[test]
    fn wallet_lock_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet_id: WalletId = 1.into();

        node.wallets.create(wallet_id);

        assert_eq!(node.wallets.valid_password(&wallet_id).unwrap(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .rpc_request(&RpcCommand::wallet_lock(wallet_id))
                .await
        });

        assert!(result.is_err());

        assert_eq!(node.wallets.valid_password(&wallet_id).unwrap(), true);

        server.abort();
    }
}
