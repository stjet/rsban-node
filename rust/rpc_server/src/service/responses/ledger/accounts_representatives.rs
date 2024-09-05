use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountsRepresentativesDto, ErrorDto};
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

pub async fn accounts_representatives(node: Arc<Node>, accounts: Vec<Account>) -> String {
    let tx = node.ledger.read_txn();
    let mut accounts_representatives: HashMap<Account, Account> = HashMap::new();

    for account in accounts {
        match node.ledger.store.account.get(&tx, &account) {
            Some(account_info) => {
                accounts_representatives.insert(account, account_info.representative.as_account());
            }
            None => return to_string_pretty(&ErrorDto::new("Account not found".to_string())).unwrap(),
        }
    }

    to_string_pretty(&AccountsRepresentativesDto::new(accounts_representatives)).unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::service::responses::{test_helpers::setup_rpc_client_and_server};
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use rsnano_rpc_messages::AccountsRepresentativesDto;
    use test_helpers::System;

    #[test]
    fn accounts_representatives() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .accounts_representatives(vec![*DEV_GENESIS_ACCOUNT])
                .await
                .unwrap()
        });

        let mut accounts_representatives = HashMap::new();
        accounts_representatives.insert(*DEV_GENESIS_ACCOUNT, *DEV_GENESIS_ACCOUNT);

        let expected = AccountsRepresentativesDto::new(accounts_representatives);
        assert_eq!(result, expected);

        server.abort();
    }
}