use rsnano_core::{Account, Amount};
use rsnano_node::Node;
use rsnano_rpc_messages::AccountBalanceDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_balance(
    node: Arc<Node>,
    account: Account,
    include_unconfirmed_blocks: Option<bool>,
) -> String {
    let tx = node.ledger.read_txn();
    let include_unconfirmed_blocks = include_unconfirmed_blocks.unwrap_or(false);

    let balance = if !include_unconfirmed_blocks {
        node.ledger
            .confirmed()
            .account_balance(&tx, &account)
            .unwrap_or(Amount::zero())
    } else {
        node.ledger
            .any()
            .account_balance(&tx, &account)
            .unwrap_or(Amount::zero())
    };

    let pending = node
        .ledger
        .account_receivable(&tx, &account, !include_unconfirmed_blocks);

    let account_balance = AccountBalanceDto::new(balance, pending, pending);

    to_string_pretty(&account_balance).unwrap()
}
