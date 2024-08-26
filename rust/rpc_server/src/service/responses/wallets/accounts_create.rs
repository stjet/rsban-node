use rsnano_core::{Account, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountsCreateDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn accounts_create(node: Arc<Node>, wallet: WalletId, count: u64) -> String {
    let mut accounts: Vec<Account> = vec![];
    for _ in 0..count as usize {
        let account = node
            .wallets
            .deterministic_insert2(&wallet, false)
            .unwrap()
            .into();
        accounts.push(account)
    }
    to_string_pretty(&AccountsCreateDto::new(accounts)).unwrap()
}
