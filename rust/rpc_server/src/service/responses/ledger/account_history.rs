use std::sync::Arc;
use rsnano_core::PublicKey;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountHistoryArgs, AccountHistoryDto, ErrorDto, HistoryEntry};
use serde_json::to_string_pretty;

pub async fn account_history(node: Arc<Node>, args: AccountHistoryArgs) -> String {
    // Extract arguments
    let account = args.account;
    let count = args.count;
    let offset = args.offset.unwrap_or(0);
    let reverse = args.reverse.unwrap_or(false);
    let raw = args.raw.unwrap_or(false);
    let head = args.head;

    let transaction = node.store.tx_begin_read();

    // Determine starting hash
    let mut hash = if let Some(head) = head {
        if node.ledger.get_block(&transaction, &head).is_some() {
            Some(head)
        } else {
            return to_string_pretty(&AccountHistoryDto {
                account: account,
                history: Vec::new(),
                previous: None,
                next: None,
            }).unwrap();
        }
    } else if reverse {
        node.ledger.account_info(&transaction, &account)
            .map(|info| info.open_block)
    } else {
        Some(node.ledger.account_info(&transaction, &account).unwrap().head)
    };

    if hash.is_none() {
        return to_string_pretty(&AccountHistoryDto {
            account: account,
            history: Vec::new(),
            previous: None,
            next: None,
        }).unwrap_or_else(|_| "{}".to_string());
    }

    let mut history = Vec::new();
    let mut remaining_count = count;
    let mut current_offset = offset;

    while let Some(current_hash) = hash {
        if remaining_count == 0 {
            break;
        }

        if let Some(block) = node.ledger.get_block(&transaction, &current_hash) {
            if current_offset > 0 {
                current_offset -= 1;
            } else {
                let mut entry = HistoryEntry {
                    hash: current_hash,
                    local_timestamp: block.sideband().unwrap().timestamp,
                    height: block.sideband().unwrap().height,
                    confirmed: node.ledger.confirmed().block_exists_or_pruned(&transaction, &current_hash),
                    work: None,
                    signature: None,
                    block_type: match block.block_type().try_into() {
                        Ok(bt) => bt,
                        Err(_) => return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap(),
                    },
                    account: block.account(),
                    amount: block.balance(),
                };

                // Add raw block data if requested
                if raw {
                    entry.work = Some(block.work().into());
                    entry.signature = Some(block.block_signature().clone());
                }

                // TODO: Implement history_visitor logic here
                // This would involve checking the block type and populating the entry accordingly

                //if !accounts_to_filter.is_empty() {
                    // TODO: Implement filtering logic
                //}

                history.push(entry);
                remaining_count -= 1;
            }
        }

        hash = if reverse {
            node.ledger.any().block_successor(&transaction, &current_hash)
        } else {
            node.ledger.get_block(&transaction, &current_hash)
                .and_then(|block| Some(block.previous()))
        };
    }

    to_string_pretty(&AccountHistoryDto {
        account: account,
        history,
        previous: if reverse { None } else { hash },
        next: if reverse { hash } else { None },
    }).unwrap()
}