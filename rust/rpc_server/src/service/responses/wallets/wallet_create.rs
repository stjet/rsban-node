use rsnano_core::WalletId;
use rsnano_rpc_messages::WalletCreatedDto;
use toml::to_string_pretty;

pub async fn wallet_create() -> String {
    let wallet = WalletId::random();

    to_string_pretty(&WalletCreatedDto::new(wallet)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn wallet_create() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone());

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create().await.unwrap() });

        let wallets = node.wallets.wallet_ids();

        assert!(wallets.contains(&result.wallet));

        server.abort();
    }
}
