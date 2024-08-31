use rsnano_core::{RawKey, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::{AccountDto, ErrorDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_add(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    raw_key: RawKey,
    work: Option<bool>,
) -> String {
    if enable_control {
        let generate_work = work.unwrap_or(false);
        match node.wallets.insert_adhoc2(&wallet, &raw_key, generate_work) {
            Ok(account) => to_string_pretty(&AccountDto::new(account.as_account())).unwrap(),
            Err(e) => to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
        }
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{PublicKey, RawKey, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn account_create_index_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        let private_key = RawKey::random();
        let public_key: PublicKey = (&private_key).try_into().unwrap();

        node.tokio.block_on(async {
            rpc_client
                .wallet_add(wallet_id, private_key, None)
                .await
                .unwrap()
        });

        assert!(node
            .wallets
            .get_accounts_of_wallet(&wallet_id)
            .unwrap()
            .contains(&public_key.into()));

        server.abort();
    }

    #[test]
    fn account_create_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let wallet_id = WalletId::random();

        node.wallets.create(wallet_id);

        let private_key = RawKey::random();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_add(wallet_id, private_key, None).await });

        assert!(result.is_err());

        server.abort();
    }
}
