use rsnano_core::{Account, BlockHash, PendingKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, ReceivableDto, WalletReceivableArgs};
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

pub async fn wallet_receivable(node: Arc<Node>, enable_control: bool, args: WalletReceivableArgs) -> String {
    if enable_control {
        let accounts = match node.wallets.get_accounts_of_wallet(&args.wallet) {
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
                .take(args.count as usize)
                .map(|(k, _)| k)
                .collect();
            pending_keys_vec.push(pending_keys);
        }

        let blocks: HashMap<Account, Vec<BlockHash>> = pending_keys_vec
            .into_iter()
            .flatten()
            .fold(HashMap::new(), |mut acc, pending_key| {
                acc.entry(pending_key.receiving_account)
                    .or_insert_with(Vec::new)
                    .push(pending_key.send_block_hash);
                acc
            });

        to_string_pretty(&ReceivableDto::new(blocks)).unwrap()
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::WalletId;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;

    #[test]
    fn wallet_receivable() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        node.wallets.create(wallet);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_receivable(wallet, 1, None, None, None, None).await.unwrap() });

        server.abort();
    }

    #[test]
    fn wallet_receivable_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_receivable(WalletId::zero(), 1, None, None, None, None).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}
