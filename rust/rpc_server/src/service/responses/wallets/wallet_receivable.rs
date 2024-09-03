use rsnano_core::{Account, BlockHash, PendingKey, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, WalletReceivableDto};
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

pub async fn wallet_receivable(
    node: Arc<Node>,
    enable_control: bool,
    wallet: WalletId,
    count: u64,
) -> String {
    if enable_control {
        let accounts = match node.wallets.get_accounts_of_wallet(&wallet) {
            Ok(accounts) => accounts,
            Err(e) => return to_string_pretty(&ErrorDto::new(e.to_string())).unwrap(),
        };

        let tx = node.ledger.read_txn();
        let mut pending_keys_vec = vec![];

        for account in accounts {
            let pending_keys: Vec<PendingKey> = node
                .ledger
                .any()
                .receivable_upper_bound(&tx, account)
                .take(count as usize)
                .map(|(k, _)| k)
                .collect();
            pending_keys_vec.push(pending_keys);
        }

        let blocks: HashMap<Account, BlockHash> = pending_keys_vec
            .into_iter()
            .flatten()
            .map(|pending_key| (pending_key.receiving_account, pending_key.send_block_hash))
            .collect();

        to_string_pretty(&WalletReceivableDto::new(blocks)).unwrap()
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
    fn wallet_receivable() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        let private_key = RawKey::zero();
        let public_key = PublicKey::try_from(&private_key).unwrap().into();

        node.wallets.create(wallet);

        node.wallets
            .insert_adhoc2(&wallet, &private_key, false)
            .unwrap();

        node.wallets.work_set(&wallet, &public_key, 1).unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_receivable(wallet, 1).await.unwrap() });

        server.abort();
    }

    #[test]
    fn wallet_receivable_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_receivable(WalletId::zero(), 1).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}
