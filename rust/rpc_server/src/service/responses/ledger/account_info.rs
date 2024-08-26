use rsnano_core::{Account, Amount};
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

    let info = node.ledger.any().get_account(&txn, &account).unwrap();

    let confirmation_height_info = node.store.confirmation_height.get(&txn, &account).unwrap();
    let account_info = AccountInfoDto::new(
        info.head,
        info.open_block,
        node.ledger.representative_block_hash(&txn, &info.head),
        info.balance.number(),
        info.modified,
        info.block_count,
        info.epoch as u8,
        confirmation_height_info.height,
        confirmation_height_info.frontier,
        None,
        None,
        None,
        None,
    );

    to_string_pretty(&account_info).unwrap()
}
