use std::sync::Arc;
use rsnano_core::{WalletId, BlockHash};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BlocksDto, ErrorDto};
use serde_json::to_string_pretty;
use std::collections::VecDeque;

pub async fn wallet_republish(node: Arc<Node>, enable_control: bool, wallet: WalletId, count: u64) -> String {
    if enable_control {
        let accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
        let mut blocks = Vec::new();
        let mut republish_bundle = Vec::new();
        let tx = node.ledger.read_txn();

        for account in accounts {
            let mut latest = node.ledger.any().account_head(&tx, &account).unwrap();
            let mut hashes = Vec::new();

            while !latest.is_zero() && hashes.len() < count as usize {
                hashes.push(latest);
                if let Some(block) = node.ledger.get_block(&tx, &latest) {
                    latest = block.previous();
                } else {
                    latest = BlockHash::zero();
                }
            }

            hashes.reverse();

            for hash in hashes {
                if let Some(block) = node.ledger.get_block(&tx, &hash) {
                    republish_bundle.push(block);
                    blocks.push(hash);
                }
            }
        }

        let republish_bundle: VecDeque<_> = republish_bundle.into();
        //node.network flood_block_many(republish_bundle, None, Duration::from_millis(25));

        to_string_pretty(&BlocksDto::new(blocks)).unwrap()
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}