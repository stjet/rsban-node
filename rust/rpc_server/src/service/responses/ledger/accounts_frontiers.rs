use rsnano_core::Account;
use rsnano_node::Node;
use rsnano_rpc_messages::{FrontiersDto, RpcDto};
use std::{collections::HashMap, sync::Arc};

pub async fn accounts_frontiers(node: Arc<Node>, accounts: Vec<Account>) -> RpcDto {
    let tx = node.ledger.read_txn();
    let mut frontiers = HashMap::new();
    let mut errors = HashMap::new();

    for account in accounts {
        if let Some(block_hash) = node.ledger.any().account_head(&tx, &account) {
            frontiers.insert(account, block_hash);
        } else {
            errors.insert(account, "Account not found".to_string());
        }
    }

    let mut frontiers_dto = FrontiersDto::new(frontiers);
    if !errors.is_empty() {
        frontiers_dto.errors = Some(errors);
    }

    RpcDto::AccountsFrontiers(frontiers_dto)
}
