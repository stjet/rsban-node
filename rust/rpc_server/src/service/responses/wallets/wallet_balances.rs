use rsnano_core::{Amount, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountBalanceDto, WalletBalancesDto};
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

use crate::service::responses::format_error_message;

pub async fn wallet_balances(
    node: Arc<Node>,
    wallet: WalletId,
    threshold: Option<Amount>,
) -> String {
    let threshold = threshold.unwrap_or(Amount::zero());
    let accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
    let mut balances = HashMap::new();
    let tx = node.ledger.read_txn();
    for account in accounts {
        let balance = match node.ledger.confirmed().account_balance(&tx, &account) {
            Some(balance) => balance,
            None => return format_error_message("Account not found"),
        };

        let pending = node.ledger.account_receivable(&tx, &account, true);

        let account_balance = AccountBalanceDto::new(balance, pending, pending);
        if balance >= threshold {
            balances.insert(account, account_balance);
        }
    }
    to_string_pretty(&WalletBalancesDto::new(balances)).unwrap()
}
