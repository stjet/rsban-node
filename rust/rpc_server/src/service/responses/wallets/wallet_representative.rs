use rsnano_core::WalletId;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountRpcMessage, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_representative(node: Arc<Node>, wallet: WalletId) -> String {
    match node.wallets.get_representative(wallet) {
        Ok(representative) => to_string_pretty(&AccountRpcMessage::new(
            "representative".to_string(),
            representative.into(),
        ))
        .unwrap(),
        Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{PublicKey, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn wallet_representative() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        node.wallets
            .set_representative(wallet, PublicKey::zero(), false)
            .unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_representative(wallet).await.unwrap() });

        assert_eq!(result.value, PublicKey::zero().into());

        server.abort();
    }
}
