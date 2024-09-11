use std::sync::Arc;
use rsnano_core::{WalletId, Account, Amount, BlockHash};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{WalletHistoryDto, HistoryEntry};
use serde_json::to_string_pretty;

pub async fn wallet_history(node: Arc<Node>, wallet: WalletId, modified_since: Option<u64>) -> String {
    let accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
    let mut entries = Vec::new();

    let block_transaction = node.store.tx_begin_read();

    for account in accounts {
        if let Some(info) = node.ledger.any().get_account(&block_transaction, &account) {
            let mut timestamp = info.modified;
            let mut hash = info.head;

            while timestamp >= modified_since.unwrap_or(0) && !hash.is_zero() {
                if let Some(block) = node.ledger.get_block(&block_transaction, &hash) {
                    let timestamp = block.sideband().map(|sideband| sideband.timestamp).unwrap_or_default();

                    if timestamp >= modified_since.unwrap_or(0) {
                        // Implement history_visitor logic here
                        // This part might need to be adapted based on the exact implementation of history_visitor
                        let entry = HistoryEntry {
                            entry_type: block.block_type().try_into().unwrap(),
                            account: block.account(),
                            amount: block.balance(),
                            block_account: account,
                            hash: hash,
                            local_timestamp: timestamp,
                        };

                        entries.push((timestamp, entry));

                        hash = block.previous();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }

    entries.sort_by(|a, b| b.0.cmp(&a.0));

    let history: Vec<HistoryEntry> = entries.into_iter().map(|(_, entry)| entry).collect();

    to_string_pretty(&WalletHistoryDto { history }).unwrap()
}