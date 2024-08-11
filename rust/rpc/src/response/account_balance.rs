use crate::format_error_message;
use rsnano_core::Account;
use rsnano_node::node::Node;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::sync::Arc;

#[derive(Serialize)]
struct AccountBalance {
    balance: String,
    pending: String,
    receivable: String,
}

impl AccountBalance {
    fn new(balance: String, pending: String, receivable: String) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}

pub(crate) async fn account_balance(
    node: Arc<Node>,
    account_str: String,
    only_confirmed: Option<bool>,
) -> String {
    let tx = node.ledger.read_txn();

    let account = match Account::decode_account(&account_str) {
        Ok(account) => account,
        Err(_) => return format_error_message("Bad account number"),
    };

    let balance = match node.ledger.confirmed().account_balance(&tx, &account) {
        Some(balance) => balance,
        None => return format_error_message("Account not found"),
    };

    let only_confirmed = only_confirmed.unwrap_or(true);

    let pending = node
        .ledger
        .account_receivable(&tx, &account, only_confirmed);

    let account_balance = AccountBalance::new(
        balance.number().to_string(),
        pending.number().to_string(),
        pending.number().to_string(),
    );

    to_string_pretty(&account_balance).unwrap()
}
