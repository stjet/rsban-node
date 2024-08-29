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

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, Amount};
    use rsnano_node::wallets::WalletsExt;
    use rsnano_rpc_messages::{AccountBalanceDto, WalletBalancesDto};
    use std::collections::HashMap;
    use test_helpers::System;

    #[test]
    fn wallet_balances_threshold_none() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        node.wallets.create(1.into());

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_balances(1.into(), None).await.unwrap() });

        let expected_balances: HashMap<Account, AccountBalanceDto> = HashMap::new();
        let expected_result = WalletBalancesDto {
            balances: expected_balances,
        };

        assert_eq!(result, expected_result);

        server.abort();
    }

    #[test]
    fn wallet_balances_threshold_some() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        node.wallets.create(1.into());

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_balances(1.into(), Some(Amount::zero()))
                .await
                .unwrap()
        });

        let expected_balances: HashMap<Account, AccountBalanceDto> = HashMap::new();
        let expected_result = WalletBalancesDto {
            balances: expected_balances,
        };

        assert_eq!(result, expected_result);

        server.abort();
    }
}
