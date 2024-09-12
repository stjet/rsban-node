use rsnano_core::{RawKey, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{ErrorDto, WalletCreateDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_create(node: Arc<Node>, enable_control: bool, seed: Option<RawKey>) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }

    let wallet = WalletId::random();
    node.wallets.create(wallet);
    let mut wallet_create_dto = WalletCreateDto::new(wallet);

    if let Some(seed) = seed {
        match node.wallets.change_seed(wallet, &seed, 0) {
            Ok((restored_count, first_account)) => {
                wallet_create_dto.last_restored_account = Some(first_account);
                wallet_create_dto.restored_count = Some(restored_count);
            }
            Err(e) => return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
        }
    }

    to_string_pretty(&wallet_create_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;
    use rsnano_core::RawKey;

    #[test]
    fn wallet_create_seed_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create(None).await.unwrap() });

        let wallets = node.wallets.wallet_ids();

        assert!(wallets.contains(&result.wallet));

        server.abort();
    }

    #[test]
    fn wallet_create_seed_some() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let seed = RawKey::from_slice(&[1u8; 32]).unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create(Some(seed)).await.unwrap() });

        let wallets = node.wallets.wallet_ids();

        assert!(wallets.contains(&result.wallet));
        assert!(result.last_restored_account.is_some());
        assert!(result.restored_count.is_some());
        assert_eq!(result.restored_count, Some(1));

        server.abort();
    }

    #[test]
    fn wallet_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_create(None).await });

        assert!(result.is_err());

        server.abort();
    }
}
