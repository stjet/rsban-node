use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountInfoDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_info(
    node: Arc<Node>,
    account: Account,
    only_confirmed: Option<bool>,
    representative: Option<bool>,
    weight: Option<bool>,
    pending: Option<bool>,
    include_confirmed: Option<bool>,
) -> String {
    let txn = node.ledger.read_txn();
    let include_confirmed = include_confirmed.unwrap_or(false);

    let info = node.ledger.any().get_account(&txn, &account).unwrap();

    let confirmation_height_info = node.store.confirmation_height.get(&txn, &account).unwrap();

    let mut account_info = AccountInfoDto::new(
        info.head,
        info.open_block,
        node.ledger.representative_block_hash(&txn, &info.head),
        info.balance,
        info.modified,
        info.block_count,
        info.epoch as u8,
    );

    account_info.confirmed_height = Some(confirmation_height_info.height);
    account_info.confirmation_height_frontier = Some(confirmation_height_info.frontier);

    to_string_pretty(&account_info).unwrap()
}
