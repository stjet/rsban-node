use std::sync::Arc;
use rsnano_core::{Account, Amount, BlockHash};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountsReceivableArgs, AccountsReceivablesDto};
use serde_json::to_string_pretty;
use std::collections::BTreeMap;

pub async fn accounts_receivable(node: Arc<Node>, args: AccountsReceivableArgs) -> String {
    let mut blocks: BTreeMap<Account, Vec<BlockHash>> = BTreeMap::new();
    let transaction = node.store.tx_begin_read();

    for account in args.accounts {
        let mut receivables: Vec<BlockHash> = Vec::new();
        let mut iter = node.ledger.any().account_receivable_upper_bound(&transaction, account, BlockHash::zero());

        while let Some((key, info)) = iter.next() {
            if receivables.len() >= args.count as usize {
                break;
            }

            if args.include_only_confirmed.unwrap_or(false) && !node.ledger.confirmed().block_exists_or_pruned(&transaction, &key.send_block_hash) {
                continue;
            }

            if info.amount < args.threshold.unwrap_or(Amount::zero()) {
                continue;
            }

            if args.source.unwrap_or(false) {
                todo!()
            } 
            else {
                receivables.push(key.send_block_hash);
            }
        }

        if !receivables.is_empty() {
            blocks.insert(account, receivables);
        }
    }

    to_string_pretty(&AccountsReceivablesDto::new("blocks".to_string(), blocks)).unwrap()
}