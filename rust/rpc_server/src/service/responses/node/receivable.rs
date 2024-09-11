use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ReceivableArgs, ReceivableDto};

pub async fn receivable(node: Arc<Node>, args: ReceivableArgs) -> ReceivableDto {
    let mut result = ReceivableDto::new();
    let transaction = node.store.tx_begin_read();
    
    let receivables = node.ledger.any().receivable_upper_bound(&transaction, args.account);
    
    for (key, info) in receivables {
        if args.include_only_confirmed.unwrap_or(true) && 
           !node.ledger.confirmed().block_exists_or_pruned(&transaction, &key.send_block_hash) {
            continue;
        }
        
        if let Some(threshold) = args.threshold {
            if info.amount < threshold {
                continue;
            }
        }
        
        result.add_block(
            key.send_block_hash,
            info.amount,
            info.source
        );
        
        if result.blocks.len() >= args.count as usize {
            break;
        }
    }
    
    // TODO: Implement sorting if args.sorting is true
    
    result
}